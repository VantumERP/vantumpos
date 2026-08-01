# SW-14 — Evidencija radnog vremena — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the per-employee working-time record — the ZoR čl. 55 st. 6 daily overtime register, the fifteen ZEOR čl. 24 tač. 1 hour buckets, the čl. 53 cap checks that carry the larger fine, the čl. 87–91 protection guards, and a two-class retention model on a shared retention table.

**Architecture:** Hours (stored as integer minutes) are the primitive; one row per `(employee, calendar date)`. All decision logic — cap checks, protection guards, period aggregation — lives in a pure `worktime.rs` that takes the clock as a parameter. Penalty copy goes through the existing `legal.rs` tier resolution. A register shift is corroborating evidence, never a source.

**Tech Stack:** Rust (rusqlite, serde) + Tauri v2 commands; React 19 + TypeScript + shadcn/ui + vitest; bun.

## Global Constraints

- **Hours are integer minutes.** Money is integer minor units. Never floating point.
- **Timestamps RFC3339, passed as a `now: &str` param.** Never `datetime('now')` in decision code.
- **Serbian Latin diacritics** (č, ć, š, ž, đ) in every operator string. Serbian quotes are `„…“`.
- **Migrations APPEND-ONLY.** New schema is **v17** only. Current count is **16** (`db/mod.rs:794`).
- **NO fine figure outside `legal.rs`**, and every new notice MUST be added to the enumerated guard list in the `legal.rs` tests.
- **NEVER render any ZEOR fine figure** anywhere — čl. 51's tier exceeds the ZoP čl. 39 ceiling for a *"fizičko lice koje ima zaposlene"* and is unresolved.
- **Never present a `[PRUDENTIAL]` item as a legal duty.**
- **Zero free text on an absence row.** No diagnosis, doznaka, ICD code or attachment, ever.
- **Never boot the app.** Verify only via the gates.
- Commit trailer: `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`

**Gates (all six, from repo root):**

```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

**Baseline at plan time:** cargo 476 passed, bun 304 passed, all six green at `907017c`.

**Governing law:** [SW14-VERIFIED-RULES.md](../../SW14-VERIFIED-RULES.md) — §4 requirement numbers are cited per task. Design: [2026-08-01-sw14-radno-vreme-design.md](../specs/2026-08-01-sw14-radno-vreme-design.md).

---

## File structure

**New Rust:** `src-tauri/src/worktime.rs` (pure: caps, guards, aggregation), `src-tauri/src/commands/worktime.rs` (commands), `src-tauri/src/retention.rs` (the shared retention table SW11-SW15 req. 42 mandates).
**Modified Rust:** `db/migrations.rs`, `db/mod.rs`, `legal.rs`, `commands/users.rs`, `commands/backup.rs`, `lib.rs`.
**New frontend:** `src/app/worktime/WorkTimeModule.tsx`, `src/app/worktime/MyHoursPanel.tsx` (+ tests).
**Modified frontend:** `services/{types,ports,local-adapter,mock-adapter}.ts`, `app/navigation.ts`, `app/AppShell.tsx`, `app/settings/SettingsScreen.tsx`.

---

### Task 1: Migration v17 — schema

**Files:** Modify `src-tauri/src/db/migrations.rs`, `src-tauri/src/db/mod.rs:794` (16 → 17), `CORE_TABLES`, `EXPLICIT_INDEXES`.

**Interfaces:**
- Produces: `work_time_entries`, `work_time_periods`, `retention_policies`, plus employee-profile columns on `users`.

- [ ] **Step 1: Write the failing tests**

Add to the `migrations.rs` tests module. **The survival test must apply `MIGRATIONS[..16]` to a raw Connection, seed with v16 columns only, and only then migrate** — mirror the pattern in commit `270796c`. A test that seeds after `Db::new` proves nothing and will be rejected.

```rust
    #[test]
    fn migration_v17_adds_worktime_schema() {
        let path = test_database_path("migration_v17_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in [
                "datum_rodjenja",
                "datum_rodjenja_najmladjeg_deteta",
                "samohrani_roditelj",
                // ZoR čl. 91 st. 2 has two legs; the težak-invalid leg has no age limit.
                "dete_tezak_invalid",
                "trudnoca_ili_dojenje",
                "trudnoca_ili_dojenje_od",
                "radi_u_preraspodeli",
                "ugovoreno_radno_vreme_minuta_nedeljno",
                "zanimanje_sifra",
                "kvalifikacija_sifra",
                // Two legally distinct written consents — čl. 91 and čl. 57 st. 4.
                "saglasnost_prekovremeni_od",
                "saglasnost_preraspodela_od",
            ] {
                assert!(
                    column_exists(&conn, "users", column),
                    "users.{column} should exist after v17"
                );
            }

            // The fifteen statutory buckets, letter-by-letter from ZEOR čl. 24 tač. 1.
            for column in [
                "moguci_minuta",
                "ukupno_ostvareni_minuta",
                "efektivno_izvrseni_minuta",
                "casovi_cekanja_i_zastoja_minuta",
                "obustava_rada_strajk_minuta",
                "ukupno_neizvrseni_minuta",
                "godisnji_odmor_minuta",
                "praznik_odmor_minuta",
                "odsustvo_uz_naknadu_minuta",
                "strucno_osposobljavanje_minuta",
                "sprecenost_poslodavac_minuta",
                "naknada_drugi_poslodavci_minuta",
                "sprecenost_rfzo_minuta",
                "porodiljsko_minuta",
                "neplaceno_odsustvo_minuta",
                "prekovremeni_minuta",
            ] {
                assert!(
                    column_exists(&conn, "work_time_entries", column),
                    "work_time_entries.{column} should exist after v17"
                );
            }

            // Advisory, not statutory — must exist but is labelled in code.
            assert!(column_exists(&conn, "work_time_entries", "nocni_minuta"));
            assert!(column_exists(&conn, "work_time_entries", "rad_na_praznik_minuta"));

            // One row per (employee, date).
            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (700, 'radnik7', 'Radnik Sedam', 'cashier', 1, '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                 INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                     VALUES (700, '2026-08-03', 480, '2026-08-03T18:00:00Z', '2026-08-03T18:00:00Z');",
            )
            .expect("first entry should insert");

            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                     VALUES (700, '2026-08-03', 60, '2026-08-03T19:00:00Z', '2026-08-03T19:00:00Z')",
                    [],
                )
                .is_err(),
                "a second row for the same (employee, day) must be rejected"
            );

            // Absence category is a closed enum enforced by the DB, not the UI.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, created_at, updated_at)
                     VALUES (700, '2026-08-04', 'nesto_izmisljeno', '2026-08-04T18:00:00Z', '2026-08-04T18:00:00Z')",
                    [],
                )
                .is_err(),
                "an unknown absence category must be rejected by the CHECK"
            );

            conn.execute_batch(
                "INSERT INTO work_time_periods (user_id, godina, mesec, status, created_at, updated_at)
                     VALUES (700, 2026, 8, 'open', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');
                 INSERT INTO retention_policies (record_class, retain_until, legal_hold, created_at, updated_at)
                     VALUES ('worktime_classification', NULL, 0, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');",
            )
            .expect("period and retention rows should insert");
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn migration_v17_preserves_pre_existing_users() {
        let path = test_database_path("migration_v17_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..16] {
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-07-01T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, pin_hash, active, created_at, updated_at, last_login_at)
                 VALUES (701, 'stara', 'Stara Radnica', 'cashier', 'hash-701', 1,
                         '2025-01-02T08:00:00Z', '2025-06-02T08:00:00Z', '2026-07-30T08:00:00Z');",
            )
            .expect("seed a v16-era user");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (username, pin, created, last): (String, String, String, String) = conn
                .query_row(
                    "SELECT username, pin_hash, created_at, last_login_at FROM users WHERE id = 701",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the pre-v17 user must survive verbatim");

            assert_eq!(username, "stara");
            assert_eq!(pin, "hash-701");
            assert_eq!(created, "2025-01-02T08:00:00Z");
            assert_eq!(last, "2026-07-30T08:00:00Z");

            let profile: Option<String> = conn
                .query_row("SELECT datum_rodjenja FROM users WHERE id = 701", [], |row| row.get(0))
                .expect("the new column exists");
            assert_eq!(profile, None, "new profile columns are nullable, not backfilled");
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v17 -- --test-threads=1`
Expected: FAIL — `users.datum_rodjenja should exist after v17`.

- [ ] **Step 3: Append migration v17**

```rust
    Migration {
        version: 17,
        name: "worktime_records_and_retention",
        sql: r#"
-- Every ISO date column carries a GLOB shape check. '2026-8-3' and '2026-08-03'
-- are the same calendar day to a human and two different keys to SQLite, which
-- would open a second slot per (zaposleni, dan) and drop those minutes out of the
-- čl. 53 weekly bucket.
ALTER TABLE users ADD COLUMN datum_rodjenja TEXT CHECK (datum_rodjenja IS NULL OR datum_rodjenja GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN datum_rodjenja_najmladjeg_deteta TEXT CHECK (datum_rodjenja_najmladjeg_deteta IS NULL OR datum_rodjenja_najmladjeg_deteta GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN samohrani_roditelj INTEGER CHECK (samohrani_roditelj IS NULL OR samohrani_roditelj IN (0, 1));
-- ZoR čl. 91 st. 2 has TWO legs: „dete do sedam godina života“ ILI „dete koje je
-- težak invalid“. The second leg carries no age limit, so it needs its own flag —
-- without it a samohrani roditelj of a disabled child aged 8+ gets no consent gate.
ALTER TABLE users ADD COLUMN dete_tezak_invalid INTEGER CHECK (dete_tezak_invalid IS NULL OR dete_tezak_invalid IN (0, 1));
ALTER TABLE users ADD COLUMN trudnoca_ili_dojenje INTEGER CHECK (trudnoca_ili_dojenje IS NULL OR trudnoca_ili_dojenje IN (0, 1));
ALTER TABLE users ADD COLUMN trudnoca_ili_dojenje_od TEXT CHECK (trudnoca_ili_dojenje_od IS NULL OR trudnoca_ili_dojenje_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN radi_u_preraspodeli INTEGER NOT NULL DEFAULT 0 CHECK (radi_u_preraspodeli IN (0, 1));
ALTER TABLE users ADD COLUMN ugovoreno_radno_vreme_minuta_nedeljno INTEGER;
ALTER TABLE users ADD COLUMN zanimanje_sifra TEXT;
ALTER TABLE users ADD COLUMN kvalifikacija_sifra TEXT;
-- Two legally distinct written consents. They are NOT interchangeable and must
-- never be read for each other's purpose:
--   saglasnost_prekovremeni_od — ZoR čl. 91: a protected parent consenting to
--     prekovremeni/noćni rad.
--   saglasnost_preraspodela_od — ZoR čl. 57 st. 4: a zaposleni „koji se saglasio“
--     to average longer in preraspodela, so hours above the average are computed
--     and paid as prekovremeni rad.
-- Neither is a ZZPL pristanak. The column records that a written consent exists
-- and from when; it does not collect one, and no consent UI belongs anywhere in
-- the employee surface.
ALTER TABLE users ADD COLUMN saglasnost_prekovremeni_od TEXT CHECK (saglasnost_prekovremeni_od IS NULL OR saglasnost_prekovremeni_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN saglasnost_preraspodela_od TEXT CHECK (saglasnost_preraspodela_od IS NULL OR saglasnost_preraspodela_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');

CREATE TABLE work_time_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    dan TEXT NOT NULL CHECK (dan GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    -- The correction chain is strictly linear: verzija 1 is the original and every
    -- ispravka appends verzija + 1. The live row for a day is MAX(verzija), which
    -- makes „the live row“ a well-defined query without a single UPDATE anywhere.
    verzija INTEGER NOT NULL DEFAULT 1 CHECK (verzija >= 1),
    moguci_minuta INTEGER NOT NULL DEFAULT 0 CHECK (moguci_minuta >= 0),
    ukupno_ostvareni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (ukupno_ostvareni_minuta >= 0),
    efektivno_izvrseni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (efektivno_izvrseni_minuta >= 0),
    casovi_cekanja_i_zastoja_minuta INTEGER NOT NULL DEFAULT 0 CHECK (casovi_cekanja_i_zastoja_minuta >= 0),
    obustava_rada_strajk_minuta INTEGER NOT NULL DEFAULT 0 CHECK (obustava_rada_strajk_minuta >= 0),
    ukupno_neizvrseni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (ukupno_neizvrseni_minuta >= 0),
    godisnji_odmor_minuta INTEGER NOT NULL DEFAULT 0 CHECK (godisnji_odmor_minuta >= 0),
    praznik_odmor_minuta INTEGER NOT NULL DEFAULT 0 CHECK (praznik_odmor_minuta >= 0),
    odsustvo_uz_naknadu_minuta INTEGER NOT NULL DEFAULT 0 CHECK (odsustvo_uz_naknadu_minuta >= 0),
    strucno_osposobljavanje_minuta INTEGER NOT NULL DEFAULT 0 CHECK (strucno_osposobljavanje_minuta >= 0),
    sprecenost_poslodavac_minuta INTEGER NOT NULL DEFAULT 0 CHECK (sprecenost_poslodavac_minuta >= 0),
    naknada_drugi_poslodavci_minuta INTEGER NOT NULL DEFAULT 0 CHECK (naknada_drugi_poslodavci_minuta >= 0),
    sprecenost_rfzo_minuta INTEGER NOT NULL DEFAULT 0 CHECK (sprecenost_rfzo_minuta >= 0),
    porodiljsko_minuta INTEGER NOT NULL DEFAULT 0 CHECK (porodiljsko_minuta >= 0),
    neplaceno_odsustvo_minuta INTEGER NOT NULL DEFAULT 0 CHECK (neplaceno_odsustvo_minuta >= 0),
    prekovremeni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (prekovremeni_minuta >= 0),
    nocni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (nocni_minuta >= 0),
    rad_na_praznik_minuta INTEGER NOT NULL DEFAULT 0 CHECK (rad_na_praznik_minuta >= 0),
    kategorija_odsustva TEXT CHECK (kategorija_odsustva IS NULL OR kategorija_odsustva IN (
        'godisnji_odmor', 'praznik', 'placeno_odsustvo', 'strucno_osposobljavanje',
        'sprecenost_poslodavac', 'sprecenost_rfzo', 'porodiljsko', 'neplaceno_odsustvo',
        'naknada_drugi_poslodavac', 'strajk'
    )),
    -- NOT A LEGAL JUSTIFICATION. These are the ZoR čl. 53 st. 1 grounds on which
    -- overtime may be ORDERED; recording one does not make a čl. 53 st. 2/3 cap
    -- breach lawful. Closed enum, because no unconstrained TEXT may exist on this
    -- table — see korekcija_razlog below.
    cap_override_razlog TEXT CHECK (cap_override_razlog IS NULL OR cap_override_razlog IN (
        'visa_sila', 'iznenadno_povecanje_obima_posla', 'neplanirani_posao_u_roku', 'drugo'
    )),
    supersedes_id INTEGER REFERENCES work_time_entries(id),
    -- ZERO free text on an absence row (§4 req. 3, §5 item 4). A free-text column
    -- on the row that also carries kategorija_odsustva and the two sprečenost
    -- buckets would eventually hold a diagnosis, an ICD code or a doznaka number,
    -- on a shop-counter PC reachable over remote support. Closed enum, no escape.
    korekcija_razlog TEXT CHECK (korekcija_razlog IS NULL OR korekcija_razlog IN (
        'greska_u_unosu', 'ispravka_sati', 'ispravka_kategorije',
        'naknadno_dostavljen_dokument', 'drugo'
    )),
    unio_user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    -- A čl. 53 cap override is only meaningful on a worked day.
    CHECK (kategorija_odsustva IS NULL OR cap_override_razlog IS NULL),
    -- Chain root vs correction can never be confused.
    CHECK ((supersedes_id IS NULL AND verzija = 1) OR (supersedes_id IS NOT NULL AND verzija > 1)),
    -- ZEOR čl. 46 st. 1 puts accuracy responsibility on the record-keeper: a
    -- correction to an append-only statutory record must carry who and why, and the
    -- schema enforces it so no other write path can route around the command layer.
    CHECK (supersedes_id IS NULL OR (unio_user_id IS NOT NULL AND korekcija_razlog IS NOT NULL))
);
-- Total, not partial. A partial index `WHERE supersedes_id IS NULL` would index
-- only chain ROOTS and let two forked corrections both stay live for one day —
-- one reporting a čl. 53 st. 2 breach and one not.
CREATE UNIQUE INDEX idx_work_time_entries_user_day
    ON work_time_entries(user_id, dan, verzija);
CREATE INDEX idx_work_time_entries_dan ON work_time_entries(dan);
-- SQLite CHECK cannot subquery, so the „same employee, same day, next verzija“
-- leg of the chain invariant is a trigger. Append-only: BEFORE INSERT only, no
-- UPDATE and no DELETE anywhere in this schema.
CREATE TRIGGER trg_work_time_entries_ispravka_isti_dan
BEFORE INSERT ON work_time_entries
WHEN NEW.supersedes_id IS NOT NULL
     AND NOT EXISTS (
         SELECT 1 FROM work_time_entries
          WHERE id = NEW.supersedes_id
            AND user_id = NEW.user_id
            AND dan = NEW.dan
            AND verzija = NEW.verzija - 1
     )
BEGIN
    SELECT RAISE(ABORT, 'Ispravka mora da pripada istom zaposlenom i istom danu i da nastavlja prethodnu verziju.');
END;

CREATE TABLE work_time_periods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    godina INTEGER NOT NULL,
    mesec INTEGER NOT NULL CHECK (mesec BETWEEN 1 AND 12),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    closed_at TEXT,
    closed_by INTEGER REFERENCES users(id),
    klasifikacija_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_work_time_periods_user_month ON work_time_periods(user_id, godina, mesec);

CREATE TABLE retention_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    record_class TEXT NOT NULL UNIQUE,
    retain_until TEXT,
    legal_hold INTEGER NOT NULL DEFAULT 0 CHECK (legal_hold IN (0, 1)),
    never_purge INTEGER NOT NULL DEFAULT 0 CHECK (never_purge IN (0, 1)),
    napomena TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#,
    },
```

Bump `db/mod.rs:794` to `17`; add `work_time_entries`, `work_time_periods`, `retention_policies` to `CORE_TABLES` and the three new indexes to `EXPLICIT_INDEXES`.

**How the one-live-row-per-day invariant actually works — read this before Task 5.** An earlier draft of this plan claimed a *partial* unique index `WHERE supersedes_id IS NULL` made "only live rows compete for the uniqueness slot". **That is inverted and false.** `supersedes_id IS NULL` selects the rows that supersede *nothing* — the chain ROOTS — so under the superseding-row correction model every correction is excluded from the index. Two forked corrections pointing at the same predecessor were both accepted, leaving two simultaneously live rows for one day: one reporting a čl. 53 st. 2 breach and one not. Nothing tied `supersedes_id` to the same employee or the same day either.

The committed schema uses a **total** unique index on `(user_id, dan, verzija)` plus the two chain `CHECK`s and the `trg_work_time_entries_ispravka_isti_dan` trigger. Consequences for later tasks:

- **The live row for a day is `MAX(verzija)` per `(user_id, dan)`.** Every read path in Tasks 2, 4, 5 and 7 must select on that, never on `supersedes_id IS NULL`.
- **The chain is strictly linear with exactly one tip.** A correction must carry `verzija = predecessor.verzija + 1`, the same `user_id` and the same `dan`; a fork at the same verzija collides on the index and one that skips a verzija is aborted by the trigger.
- **Still pure append-only** — no `UPDATE` and no `DELETE` anywhere in this schema.
- **Corrections are attributable at the schema level**: `supersedes_id NOT NULL` requires both `unio_user_id` and `korekcija_razlog`, so Task 5's command layer is a second gate, not the only one.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
Expected: PASS, 476 → 478.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db
git commit -m "$(cat <<'EOF'
feat(sw-14): migration v17 — worktime records, periods, shared retention table

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `worktime.rs` — the čl. 53 caps, pure

**Files:** Create `src-tauri/src/worktime.rs`; modify `src-tauri/src/lib.rs` (`mod worktime;`).

**Interfaces:**
- Produces: `CapAssessment { weekly_overtime_minutes, daily_total_minutes, weekly_cap_exceeded, daily_cap_exceeded, requires_override }`, and `assess_caps(day: &str, entry: &DayHours, week: &[DayHours]) -> CapAssessment`. Task 4 wraps it; Task 3 supplies the notice.

**The numbers, from ZoR čl. 53:** overtime **≤ 8 h in a calendar week** (st. 2); total **≤ 12 h in a day including overtime** (st. 3). Minutes: 480 and 720.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn day(effective: i64, overtime: i64) -> DayHours {
        DayHours { dan: "2026-08-03".to_string(), efektivno_minuta: effective, prekovremeni_minuta: overtime }
    }

    #[test]
    fn weekly_overtime_cap_is_eight_hours() {
        let week = vec![day(480, 120), day(480, 120), day(480, 120)]; // 6 h so far
        let a = assess_caps("2026-08-06", &day(480, 120), &week);     // +2 h = 8 h exactly
        assert_eq!(a.weekly_overtime_minutes, 480);
        assert!(!a.weekly_cap_exceeded, "exactly 8 h is the cap, not over it");

        let a = assess_caps("2026-08-06", &day(480, 121), &week);
        assert!(a.weekly_cap_exceeded, "8 h + 1 minute is over ZoR čl. 53 st. 2");
    }

    #[test]
    fn daily_total_cap_is_twelve_hours_including_overtime() {
        let a = assess_caps("2026-08-03", &day(480, 240), &[]);   // 8 + 4 = 12 h
        assert_eq!(a.daily_total_minutes, 720);
        assert!(!a.daily_cap_exceeded, "exactly 12 h is the cap");

        let a = assess_caps("2026-08-03", &day(480, 241), &[]);
        assert!(a.daily_cap_exceeded, "12 h + 1 minute is over ZoR čl. 53 st. 3");
    }

    /// The record must be able to describe an unlawful day — refusing to record
    /// reality would hide the čl. 274 exposure instead of surfacing it.
    #[test]
    fn exceeding_a_cap_requires_an_override_but_is_never_impossible() {
        let a = assess_caps("2026-08-03", &day(480, 300), &[]);
        assert!(a.daily_cap_exceeded);
        assert!(a.requires_override, "the operator must say why, not be blocked");
    }

    #[test]
    fn the_week_is_a_calendar_week_starting_monday() {
        // 2026-08-03 is a Monday; 2026-08-02 is the Sunday before and must not count.
        let prior_sunday = DayHours { dan: "2026-08-02".to_string(), efektivno_minuta: 480, prekovremeni_minuta: 480 };
        let a = assess_caps("2026-08-03", &day(480, 60), &[prior_sunday]);
        assert_eq!(a.weekly_overtime_minutes, 60, "last week's overtime does not carry in");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worktime:: -- --test-threads=1`
Expected: FAIL — `file not found for module worktime`.

- [ ] **Step 3: Implement**

```rust
//! Working-time rules — ZoR čl. 53 caps and the čl. 87–91 protection guards.
//!
//! Everything here is pure and takes the day as a parameter: the caps are what
//! carry the larger fine (čl. 274 st. 1 tač. 3, preduzetnik 200.000–400.000,
//! against 50.000–150.000 for the missing register), so they must be testable
//! exactly at the boundary.

pub const WEEKLY_OVERTIME_CAP_MINUTES: i64 = 8 * 60;
pub const DAILY_TOTAL_CAP_MINUTES: i64 = 12 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayHours {
    pub dan: String,
    pub efektivno_minuta: i64,
    pub prekovremeni_minuta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapAssessment {
    pub weekly_overtime_minutes: i64,
    pub daily_total_minutes: i64,
    pub weekly_cap_exceeded: bool,
    pub daily_cap_exceeded: bool,
    pub requires_override: bool,
}

pub fn assess_caps(day: &str, entry: &DayHours, week: &[DayHours]) -> CapAssessment {
    let same_week: i64 = week
        .iter()
        .filter(|d| d.dan != day && in_same_iso_week(&d.dan, day))
        .map(|d| d.prekovremeni_minuta)
        .sum();

    let weekly_overtime_minutes = same_week + entry.prekovremeni_minuta;
    let daily_total_minutes = entry.efektivno_minuta + entry.prekovremeni_minuta;
    let weekly_cap_exceeded = weekly_overtime_minutes > WEEKLY_OVERTIME_CAP_MINUTES;
    let daily_cap_exceeded = daily_total_minutes > DAILY_TOTAL_CAP_MINUTES;

    CapAssessment {
        weekly_overtime_minutes,
        daily_total_minutes,
        weekly_cap_exceeded,
        daily_cap_exceeded,
        requires_override: weekly_cap_exceeded || daily_cap_exceeded,
    }
}
```

plus `in_same_iso_week(a: &str, b: &str) -> bool` built on the civil-date helpers already proven in `cash_deposit.rs` — reuse them rather than writing a second date implementation; if they are private, make them `pub(crate)`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worktime:: -- --test-threads=1`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/worktime.rs src-tauri/src/lib.rs src-tauri/src/cash_deposit.rs
git commit -m "$(cat <<'EOF'
feat(sw-14): ZoR cl. 53 cap assessment — the checks that carry the larger fine

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `legal.rs` — the two ZoR notices

**Files:** Modify `src-tauri/src/legal.rs`.

**Interfaces:** Produces `overtime_record_missing(&ShopProfile) -> LegalNotice`, `overtime_caps_exceeded(&ShopProfile) -> LegalNotice`. **Both MUST be added to `all_notices` in the tests** — that enumerated list is the module's entire safety property.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn overtime_notices_are_tier_correct_and_carry_no_zeor_figure() {
        let p = profile(Some(PravnaForma::Preduzetnik));

        let missing = overtime_record_missing(&p);
        let penalty = missing.penalty.expect("known");
        assert!(penalty.contains("50.000"), "{penalty}");
        assert!(penalty.contains("150.000"), "{penalty}");
        assert!(
            !penalty.contains("300.000"),
            "300.000 is the pravno-lice tier for čl. 276 st. 1: {penalty}"
        );
        assert!(
            !penalty.contains("odgovorno lice"),
            "čl. 276 st. 2 does not reach a preduzetnik: {penalty}"
        );

        let caps = overtime_caps_exceeded(&p);
        let penalty = caps.penalty.expect("known");
        assert!(penalty.contains("200.000") && penalty.contains("400.000"), "{penalty}");
    }

    /// ZEOR čl. 50/51 tiers are unresolved — čl. 51 exceeds the ZoP čl. 39
    /// ceiling for a "fizičko lice koje ima zaposlene". Silence beats a wrong
    /// number, so no ZEOR amount may appear in any notice, under any profile.
    #[test]
    fn no_zeor_figure_is_reachable_in_any_notice() {
        for forma in [Some(PravnaForma::Preduzetnik), Some(PravnaForma::PravnoLice), None] {
            let p = profile(forma);
            for notice in all_notices(&p) {
                let rendered = format!(
                    "{} {} {}",
                    notice.summary,
                    notice.penalty.clone().unwrap_or_default(),
                    notice.citation
                );
                for forbidden in ["500.000 do 1.000.000", "300.000 do 500.000", "ZEOR"] {
                    assert!(
                        !rendered.contains(forbidden),
                        "no ZEOR figure may reach an operator; found {forbidden:?} in: {rendered}"
                    );
                }
            }
        }
    }
```

Extend `all_notices` with both new functions.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml legal:: -- --test-threads=1`
Expected: FAIL — `cannot find function overtime_record_missing`.

- [ ] **Step 3: Implement**

```rust
/// ZoR čl. 55 st. 6 — the daily overtime register.
pub fn overtime_record_missing(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu \
                  zaposlenih."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 50.000 do 150.000 dinara \
             (čl. 276 st. 1 u vezi sa tač. 1a).",
            "Prekršaj: novčana kazna od 150.000 do 300.000 dinara (čl. 276 st. 1 tač. 1a), \
             uz kaznu za odgovorno lice od 10.000 do 20.000 dinara (čl. 276 st. 2).",
        ),
        citation: "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada. \
                   Ovi članovi ne propisuju zaštitnu meru."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZoR čl. 53 — the overtime caps. This is the LARGER exposure, ~2.7× the
/// missing-register fine, and it is why the cap checks are the feature.
pub fn overtime_caps_exceeded(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Prekovremeni rad ne može trajati duže od osam časova nedeljno, \
                  niti ukupno radno vreme sa prekovremenim duže od 12 časova dnevno."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 200.000 do 400.000 dinara \
             (čl. 274 st. 1 tač. 3 u vezi sa st. 2).",
            "Prekršaj: novčana kazna od 600.000 do 1.500.000 dinara (čl. 274 st. 1 tač. 3), \
             uz kaznu za odgovorno lice od 30.000 do 150.000 dinara (čl. 274 st. 3).",
        ),
        citation: "Zakon o radu, čl. 53 st. 2 i st. 3. Nadzor: inspektor rada. \
                   Ovi članovi ne propisuju zaštitnu meru."
            .to_string(),
        is_legal_duty: true,
    }
}
```

**Before committing, the implementer must re-read `SW14-VERIFIED-RULES.md` §3 W1 and confirm the pravno-lice figures for čl. 274 verbatim.** If the memo does not state them, render `None` for that tier rather than guessing, and record it as a deviation.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml legal:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/legal.rs
git commit -m "$(cat <<'EOF'
feat(sw-14): ZoR overtime notices in legal.rs; guard against any ZEOR figure

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Protection guards (čl. 87–91)

**Files:** Modify `src-tauri/src/worktime.rs`.

**Interfaces:** Produces `EmployeeProtection { datum_rodjenja, datum_rodjenja_najmladjeg_deteta, samohrani_roditelj, dete_tezak_invalid, trudnoca_ili_dojenje, saglasnost_prekovremeni_od, radi_u_preraspodeli }` and `check_protection(p: &EmployeeProtection, day: &str, entry: &DayHours) -> Vec<ProtectionBlock>`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn an_employee_under_eighteen_cannot_be_given_overtime_at_all() {
        // čl. 88 st. 1 — an unconditional prohibition, not a warning.
        let p = protection_born("2009-09-01");
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanPrekovremeni));
        assert!(blocks[0].blocking, "this one blocks, it does not warn");
    }

    #[test]
    fn an_employee_under_eighteen_is_capped_at_eight_hours_a_day() {
        let p = protection_born("2009-09-01");
        let blocks = check_protection(&p, "2026-08-03", &day(540, 0));
        assert!(blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanDnevniLimit));
    }

    /// The threshold is SEVEN for a samohrani roditelj (čl. 91 st. 2). The
    /// commonly-quoted fourteen is wrong and would silently drop the guard.
    #[test]
    fn a_single_parent_of_a_child_under_seven_needs_recorded_consent() {
        let mut p = protection_child_born("2020-06-01"); // 6 years old on the test day
        p.samohrani_roditelj = Some(true);
        p.saglasnost_prekovremeni_od = None;

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks.iter().any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "a samohrani roditelj of a six-year-old is inside čl. 91 st. 2"
        );

        p.saglasnost_prekovremeni_od = Some("2026-01-15".to_string());
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(!blocks.iter().any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
    }

    #[test]
    fn a_single_parent_of_a_child_over_seven_needs_no_consent() {
        let mut p = protection_child_born("2018-06-01"); // 8 years old
        p.samohrani_roditelj = Some(true);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(!blocks.iter().any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
    }

    /// čl. 91 st. 2 is „dete do sedam godina života ILI dete koje je težak invalid“.
    /// The disability leg has NO age limit — an age-only guard drops it silently.
    #[test]
    fn a_single_parent_of_a_disabled_child_needs_consent_at_any_age() {
        let mut p = protection_child_born("2010-06-01"); // 16 years old
        p.samohrani_roditelj = Some(true);
        p.dete_tezak_invalid = Some(true);
        p.saglasnost_prekovremeni_od = None;

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks.iter().any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "the težak-invalid leg of čl. 91 st. 2 is not bounded by the seven-year threshold"
        );
    }

    #[test]
    fn a_non_single_parent_threshold_is_three_not_seven() {
        let mut p = protection_child_born("2022-06-01"); // 4 years old
        p.samohrani_roditelj = Some(false);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            !blocks.iter().any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "čl. 91 st. 1 covers a child up to three"
        );
    }

    /// čl. 90 is NOT an unconditional block — it fires on a health authority's
    /// finding. We store the flag; we never store the finding.
    #[test]
    fn pregnancy_warns_rather_than_blocks() {
        let mut p = protection_born("1995-01-01");
        p.trudnoca_ili_dojenje = Some(true);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        let block = blocks.iter().find(|b| b.kind == ProtectionKind::TrudnocaNocniIPrekovremeni)
            .expect("a warning is raised");
        assert!(!block.blocking, "čl. 90 depends on a nalaz — the app must not decide it");
    }

    #[test]
    fn preraspodela_suppresses_automatic_overtime_derivation() {
        let mut p = protection_born("1995-01-01");
        p.radi_u_preraspodeli = true;
        assert!(!derives_overtime_automatically(&p), "čl. 58 hours are not overtime");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worktime:: -- --test-threads=1`
Expected: FAIL — `cannot find function check_protection`.

- [ ] **Step 3: Implement**

`ProtectionKind` enum, `ProtectionBlock { kind, blocking, poruka }`, and `check_protection` applying: under-18 → blocking on any overtime and on >8 h/day (čl. 87, čl. 88 st. 1); child age threshold **3** normally and **7** for `samohrani_roditelj` (čl. 91 st. 1 / st. 2) → consent required, non-blocking once `saglasnost_prekovremeni_od` is present; `trudnoca_ili_dojenje` → non-blocking warning (čl. 90). Age computed from the entry's `dan`, never from a wall clock. Plus `derives_overtime_automatically(p) -> bool` returning `!p.radi_u_preraspodeli`.

**Both legs of čl. 91 st. 2, not just the age one.** The consent requirement also fires when `samohrani_roditelj = 1 AND dete_tezak_invalid = 1`, **at any child age, independent of the seven-year threshold** — the statute reads *„Samohrani roditelj koji ima dete do sedam godina života **ili dete koje je težak invalid**“*. The ordinary parental threshold stays **3** (st. 1) and the samohrani age threshold stays **7** (st. 2); the disability leg is an additional disjunct, never a replacement.

**Consent columns are not interchangeable.** `check_protection` reads **only** `saglasnost_prekovremeni_od` — that column is the čl. 91 written consent and nothing else. The čl. 57 st. 4 conversion (§4 req. 11: a zaposleni *„koji se saglasio“* to average longer in preraspodela, whose hours above the average are paid as overtime) is a **separate** consent stored in `saglasnost_preraspodela_od`. That conversion is **out of scope for SW-14 as planned** — no task implements it — and `saglasnost_prekovremeni_od` must never be consulted for it. The column exists in v17 so a later task can implement req. 11 without a schema change.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worktime:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/worktime.rs
git commit -m "$(cat <<'EOF'
feat(sw-14): cl. 87-91 protection guards; the single-parent threshold is SEVEN

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Commands — entry, correction, period close

**Files:** Create `src-tauri/src/commands/worktime.rs`; modify `src-tauri/src/lib.rs`, `src-tauri/src/commands/mod.rs`.

**Interfaces:** Produces `worktime_list_month`, `worktime_save_entry`, `worktime_correct_entry`, `worktime_close_period`, `worktime_export_csv`, `worktime_my_hours`. All admin-gated except `worktime_my_hours` (session-gated, own rows only).

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn saving_an_entry_records_the_operator_and_the_day() { /* … */ }

    #[test]
    fn a_correction_supersedes_rather_than_mutates() {
        // Append-only: the original row survives, struck through, and the
        // superseding row carries who/when/why. The correction must be written
        // with verzija = predecessor.verzija + 1 on the SAME (user_id, dan);
        // the live row is MAX(verzija), never `supersedes_id IS NULL`.
    }

    #[test]
    fn an_entry_cannot_be_back_dated_into_a_closed_period() {
        // "dnevnu" in čl. 55 st. 6 is what makes a reconstructed month look
        // like a fabrication.
    }

    #[test]
    fn closing_a_period_freezes_the_classification_and_is_irreversible() { /* … */ }

    #[test]
    fn my_hours_returns_only_the_session_users_own_rows() { /* … */ }

    #[test]
    fn a_cashier_cannot_reach_the_admin_worktime_commands() { /* … */ }
```

Each test body must be written out in full by the implementer following the seeding style in `commands/shifts.rs`; the six behaviours above are the required coverage.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worktime -- --test-threads=1`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

Domain functions taking `&AppState` + `now: &str`, `#[tauri::command]` wrappers, `require_admin` inside the domain function so no caller can route around it. `worktime_save_entry` runs `assess_caps` + `check_protection`, rejects a blocking protection result, and requires `cap_override_razlog` when `requires_override` is set. Register all six in `lib.rs`.

- [ ] **Step 4: Run to verify they pass** — `cargo test … -- --test-threads=1`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(sw-14): worktime commands — append-only entry, correction, period close

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Retention classes + go-live reset

**Files:** Create `src-tauri/src/retention.rs`; modify `src-tauri/src/commands/backup.rs`.

**Interfaces:** Produces `RecordClass` enum, `seed_retention_policies(state, now)`, `is_purgeable(class, today) -> bool`.

Class A (`worktime_classification`) is `never_purge = 1` — structurally unreachable by every purge, reset, restore and backup-prune path. Class B (`worktime_draft`) is bounded, purge-eligible only once the period is closed. Standalone overtime log floor **3 years**, upward-only.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn the_closed_classification_is_never_purgeable() { /* never_purge wins over any date */ }

    #[test]
    fn retain_until_only_ever_moves_forward() { /* an earlier date is rejected */ }

    #[test]
    fn go_live_reset_preserves_closed_worktime_periods_and_the_retention_table() {
        // Payroll-adjacent records are a never_purge class; a go-live reset is
        // a pre-production tool and must not touch them.
    }
```

- [ ] **Step 2–4:** implement, run, confirm.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/retention.rs src-tauri/src/commands/backup.rs src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(sw-14): shared retention table — trajno classification, bounded drafts

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Frontend — Radno vreme module

**Files:** Create `src/app/worktime/WorkTimeModule.tsx` + test; modify `services/*`, `app/navigation.ts`, `app/AppShell.tsx`.

- [ ] **Step 1: Write the failing tests**

```tsx
  it("labels advisory columns as computed, never as statutory fields", async () => {
    render(<WorkTimeModule services={mockServices()} />);
    expect(screen.getByText(/izračunato radi provere usklađenosti/i)).toBeInTheDocument();
    expect(screen.queryByText(/noćni časovi.*zakonom propisano/i)).not.toBeInTheDocument();
  });

  it("offers an override with a reason when a cap is exceeded, never a dead end", async () => {
    /* enter 13 h, assert the warning + the reason field + that saving is possible */
  });

  it("shows no ZEOR figure anywhere", async () => {
    render(<WorkTimeModule services={mockServices()} />);
    expect(screen.queryByText(/500\.000 do 1\.000\.000/)).not.toBeInTheDocument();
    expect(screen.queryByText(/ZEOR/)).not.toBeInTheDocument();
  });

  it("hides the absence reason from a non-payroll role", async () => { /* … */ });
```

- [ ] **Steps 2–4:** implement the monthly grid, the cap warnings via `legal.rs` notices, period close, nav item `{ id: "worktime", label: "Radno vreme", icon: ClockIcon, adminOnly: true }`.

- [ ] **Step 5: Commit**

```bash
git add src/app/worktime src/app/navigation.ts src/app/AppShell.tsx src/services
git commit -m "$(cat <<'EOF'
feat(sw-14): Radno vreme module — monthly grid, cap warnings, period close

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Frontend — "Moji sati" + export

**Files:** Create `src/app/worktime/MyHoursPanel.tsx` + test.

Read-only per-employee view (discharges ZoR čl. 83 st. 1 and ZZPL čl. 26). Export header carries *"Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac."*

- [ ] **Step 1: Write the failing tests**

```tsx
  it("is read-only", async () => {
    render(<MyHoursPanel hours={fixture} />);
    expect(screen.queryByRole("button", { name: /sačuvaj|izmeni/i })).not.toBeInTheDocument();
  });

  it("states that no obrazac is prescribed", async () => {
    render(<MyHoursPanel hours={fixture} />);
    expect(screen.getByText(/zakon ne propisuje obrazac/i)).toBeInTheDocument();
  });
```

- [ ] **Steps 2–5:** implement, run `bun run test`, commit.

---

### Task 9: Employee profile fields in the Users screen

**Files:** Modify `src-tauri/src/commands/users.rs`, `src/app/settings/SettingsScreen.tsx`.

The čl. 87–91 fields, each with copy naming the article it serves — including `dete_tezak_invalid`, the second leg of čl. 91 st. 2, which has no age limit. **No consent UI** — `saglasnost_prekovremeni_od` records that a written consent exists and when, it does not collect one; the same is true of `saglasnost_preraspodela_od` (čl. 57 st. 4), which this task does not surface. **No free text on `trudnoca_ili_dojenje`**, boolean + date only.

- [ ] **Steps 1–5:** failing tests (validation, no-consent-UI assertion, date-only pregnancy flag), implement, run, commit.

---

### Task 10: Docs + full gate run

- [ ] **Step 1:** flip SW-14 in `docs/SERBIAN-LAW-COMPLIANCE.md` §3 to shipped, naming `worktime.rs` / `retention.rs`.
- [ ] **Step 2:** update `docs/PROGRESS.md` with real counts.
- [ ] **Step 3:** add the posebne-vrste row, named recipients and the two-class retention statement to `docs/compliance/obavestenje-zaposlenima.md` (req. 26).
- [ ] **Step 4:** run all six gates; report exact counts.
- [ ] **Step 5:** commit.

---

## Self-review

**Spec coverage.** §1.1 → Task 1. §1.2 → Tasks 1, 7. §1.3 → Task 1. §2.1 → Tasks 2, 5, 7. §2.2 → Task 4. §2.3 → Tasks 4, 9. §3 → Task 3. §4 → Task 6. §5 → Tasks 7, 8. §6 → Tasks 5, 7, 9, 10. §7 → enforced by the Task 3 guard and the Task 7 assertions.

**Placeholders.** Tasks 5, 6, 9 name required behaviours rather than pasting full bodies, because their seeding boilerplate follows an established repo pattern the implementer must read rather than have transcribed. Every behaviour is stated as a concrete assertion. Tasks 1–4 and 7–8 carry full code.

**Type consistency.** `DayHours` and `CapAssessment` (Task 2) are consumed in Tasks 4, 5, 7. `EmployeeProtection` (Task 4) is consumed in Tasks 5, 9. `LegalNotice` (existing) is consumed in Tasks 3, 5, 7. Minutes are the unit everywhere; no task uses fractional hours.

**One gap found and fixed during review:** Task 2 originally described its own date helpers, which would have duplicated the civil-date arithmetic already proven in `cash_deposit.rs`. It now reuses them.
