# Reklamacija no-fee ban (čl. 63 st. 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 35/2026 čl. 63 st. 3 — *„Zabranjeno je da trgovac naplaćuje utvrđivanje nesaobraznosti"* — as an operator-facing prohibition plus a stored attestation gating the answer and resolve transitions, NEW regime only.

**Architecture:** A v16 migration adds `no_fee_attested` / `no_fee_attested_at` to `reklamacije`. `get_reklamacija` derives `no_fee_notice: Option<String>` from the record's frozen regime, so every renderer gates on `Option` presence instead of testing the regime itself. One idempotent helper, `ensure_no_fee_attested`, is called inside the existing transactions of `log_answer` and `resolve_reklamacija`.

**Tech Stack:** Rust (rusqlite, serde, Tauri v2 commands), React + TypeScript (vitest, @testing-library/react), SQLite.

**Spec:** `docs/superpowers/specs/2026-08-02-reklamacija-no-fee-ban-design.md`
**Legal authority:** `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md` §4(c), §5, §6.

## Global Constraints

- **TDD is mandatory.** Write the failing test, run it, confirm it fails *for the right reason*, then write the minimal implementation. A step that says "run it and see it fail" is not optional.
- **Never boot the app.** No `tauri dev`, no preview server, no binary. Verify through tests and the build only.
- **Migrations are append-only.** Never edit an existing `Migration` entry; only add a new one.
- **Serbian Latin diacritics in every operator-facing string** — č, ć, š, ž, đ. `Zabranjeno je naplatiti utvrđivanje nesaobraznosti`, not `utvrdjivanje`.
- **No fine figure may live outside `src-tauri/src/legal.rs`.** Nothing in this plan adds one; the prohibition text carries a citation, not an amount.
- **The ban is NEW-regime only.** 88/2021 čl. 55 st. 3 has no fee ban. Nothing may gate, prompt, or inform an OLD-regime record about it.
- **Commit trailer, exactly:** `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`
- **Gate commands** (all must pass before the final commit of each task that touches the relevant side):
  - `bun run test`
  - `bun run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - `git diff --check`

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `src-tauri/src/db/migrations.rs` | v16 entry + migration tests | 1 |
| `src-tauri/src/reklamacije.rs` | view fields, `NO_FEE_NOTICE`, `MSG_NEW_NO_FEE`, `ensure_no_fee_attested`, gate wiring | 1, 2 |
| `src-tauri/src/commands/reklamacije.rs` | `reklamacija_resolve` parameter | 2 |
| `src-tauri/src/legal.rs` | widen `reklamacija_breach` summary | 3 |
| `src-tauri/src/reklamacije_docs.rs` | potvrda (gated) + prodajno-mesto notice (unconditional) | 4 |
| `src/services/types.ts`, `ports.ts`, `local-adapter.ts`, `mock-adapter.ts` | contract + doubles | 5 |
| `src/app/reklamacije/ReklamacijeModule.tsx` | prohibition line + attestation checkbox | 6 |

---

### Task 1: Migration v16 and the view fields

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append a `Migration` after the `version: 15` entry, whose closing `},` is line 570; add tests in the `mod tests` block)
- Modify: `src-tauri/src/reklamacije.rs` (`MSG_NEW_ANSWER_WARNING` at line 602; `ReklamacijaView` at line 277; `get_reklamacija` at line 429)

**Interfaces:**
- Consumes: nothing.
- Produces: `ReklamacijaView.no_fee_attested: bool`, `ReklamacijaView.no_fee_attested_at: Option<String>`, `ReklamacijaView.no_fee_notice: Option<String>`, and `pub const NO_FEE_NOTICE: &str` in `crate::reklamacije`.

- [ ] **Step 1: Write the failing migration test**

Add to the `mod tests` block in `src-tauri/src/db/migrations.rs`, after `migration_v15_adds_declaration_and_cash_compliance_schema`:

```rust
    #[test]
    fn migration_v16_adds_the_no_fee_attestation_columns() {
        let path = test_database_path("migration_v16_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in ["no_fee_attested", "no_fee_attested_at"] {
                assert!(
                    column_exists(&conn, "reklamacije", column),
                    "reklamacije.{column} should exist after v16"
                );
            }

            // A record predating the gate was never asked, so it must read as
            // unattested rather than as silently compliant.
            conn.execute(
                "INSERT INTO reklamacije (
                    register_number, regime, status, filed_at, podnosilac_ime_prezime,
                    podaci_o_robi, opis_nesaobraznosti, zahtev, roba_kind,
                    datum_izdavanja_potvrde, created_at, updated_at
                 )
                 VALUES (901, 'new', 'open', '2026-08-15T00:00:00Z', 'Petar Petrović',
                         'Veš mašina', 'Ne centrifugira', 'Popravka', 'tehnicka',
                         '2026-08-15T10:00:00Z', '2026-08-15T10:00:00Z', '2026-08-15T10:00:00Z')",
                [],
            )
            .expect("a reklamacija row should insert under v16");

            let (attested, attested_at): (i64, Option<String>) = conn
                .query_row(
                    "SELECT no_fee_attested, no_fee_attested_at FROM reklamacije
                     WHERE register_number = 901",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the seeded row should read back");
            assert_eq!(attested, 0, "the default must be UNattested");
            assert!(attested_at.is_none());
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }
```

- [ ] **Step 2: Run it and confirm it fails for the right reason**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib migration_v16 -- --test-threads=1`
Expected: FAIL — `reklamacije.no_fee_attested should exist after v16`. If it fails on the INSERT instead, the column list is wrong; fix the test before continuing.

- [ ] **Step 3: Append the v16 migration**

In `src-tauri/src/db/migrations.rs`, immediately after the closing `},` of the `version: 15` entry and before the closing `];` of `MIGRATIONS`:

```rust
    Migration {
        version: 16,
        name: "reklamacija_no_fee_attestation",
        sql: r#"
-- 35/2026 čl. 63 st. 3: „Zabranjeno je da trgovac naplaćuje utvrđivanje
-- nesaobraznosti." Attested once per complaint, NEW regime only — 88/2021
-- čl. 55 st. 3 carries no such ban, so old-regime rows stay 0 forever and that
-- 0 must never be read as a failure to comply with a duty that never bound them.
ALTER TABLE reklamacije ADD COLUMN no_fee_attested INTEGER NOT NULL DEFAULT 0
    CHECK (no_fee_attested IN (0, 1));
ALTER TABLE reklamacije ADD COLUMN no_fee_attested_at TEXT;
"#,
    },
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib migration_v16 -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Write the failing view test**

Add to the `mod tests` block in `src-tauri/src/reklamacije.rs`, after `view_carries_the_regime_and_tier_correct_penalty_notice`:

```rust
    #[test]
    fn no_fee_notice_is_new_regime_only_and_starts_unattested() {
        with_reklamacija_db("rek_no_fee_notice", |conn| {
            // 88/2021 čl. 55 st. 3 has no fee ban. Showing one to a pre-cutover
            // complaint asserts a duty that does not bind it (memo §6).
            let old = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(old.regime, REGIME_OLD);
            assert!(
                old.no_fee_notice.is_none(),
                "the old regime must carry no fee-ban copy: {:?}",
                old.no_fee_notice
            );
            assert!(!old.no_fee_attested);
            assert!(old.no_fee_attested_at.is_none());

            let new = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(new.regime, REGIME_NEW);
            let notice = new.no_fee_notice.expect("the new regime carries the ban");
            assert!(notice.contains("utvrđivanje nesaobraznosti"), "{notice}");
            assert!(notice.contains("čl. 63 st. 3"), "{notice}");
            // The neighbouring rule must be named, not merely fenced off, or the
            // operator can read this as "the repair may be charged for".
            assert!(notice.contains("čl. 56 st. 1"), "{notice}");
            assert!(!new.no_fee_attested, "intake attests nothing");
            assert!(new.no_fee_attested_at.is_none());
        });
    }
```

- [ ] **Step 6: Run it and confirm it fails for the right reason**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib no_fee_notice_is_new_regime_only -- --test-threads=1`
Expected: FAIL to compile — `no field 'no_fee_notice' on type 'ReklamacijaView'`.

- [ ] **Step 7: Add the const**

In `src-tauri/src/reklamacije.rs`, immediately after `MSG_NEW_ANSWER_WARNING` (line 602):

```rust
/// 35/2026 čl. 63 st. 3, second sentence, and the neighbouring rule it is most
/// often confused with. The second sentence is load-bearing: fencing the ban off
/// from the manner of resolution *without* naming what governs there invites the
/// reading that the repair may be charged for. It may not, in either regime.
pub const NO_FEE_NOTICE: &str = "Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). \
     Otklanjanje nesaobraznosti — popravka ili zamena — je bez naknade po posebnoj odredbi \
     (čl. 56 st. 1), i u starom i u novom režimu.";
```

- [ ] **Step 8: Add the three view fields**

In `ReklamacijaView` (line 277), after `pub notice: LegalNotice,`:

```rust
    /// 35/2026 čl. 63 st. 3 attestation. Meaningless under the OLD regime, which
    /// carries no fee ban — an old-regime record stays `false` forever and that
    /// `false` is not a compliance signal.
    pub no_fee_attested: bool,
    pub no_fee_attested_at: Option<String>,
    /// `Some` iff this record's frozen regime is NEW. Renderers gate on presence,
    /// so no surface has to re-derive the regime and none can get it wrong.
    pub no_fee_notice: Option<String>,
```

- [ ] **Step 9: Read and derive them in `get_reklamacija`**

In `get_reklamacija` (line 429), extend the SELECT and the tuple. The tuple type gains two entries at the end:

```rust
        Option<i64>,
        String,
        String,
        i64,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT id, register_number, regime, status, filed_at, podnosilac_ime_prezime,
                    kontakt, podaci_o_robi, opis_nesaobraznosti, zahtev, roba_kind,
                    datum_izdavanja_potvrde, created_by, created_at, updated_at,
                    no_fee_attested, no_fee_attested_at
             FROM reklamacije
             WHERE id = ?1",
```

Add `row.get(15)?,` and `row.get(16)?,` to the closure's tuple, and extend the destructuring binding with `no_fee_attested,` and `no_fee_attested_at,` after `updated_at,`.

Then, next to the existing `notice` derivation:

```rust
    // Gated on the record's OWN frozen regime, never on today's date: a complaint
    // filed before the cutover never acquires the ban, whatever the clock says.
    let no_fee_notice = (regime == REGIME_NEW).then(|| NO_FEE_NOTICE.to_string());
```

And in the returned struct literal, after `notice,`:

```rust
        no_fee_attested: no_fee_attested != 0,
        no_fee_attested_at,
        no_fee_notice,
```

- [ ] **Step 10: Run it and confirm it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reklamacije -- --test-threads=1`
Expected: PASS, all reklamacije tests green.

- [ ] **Step 11: Format, lint, commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
git add src-tauri/src/db/migrations.rs src-tauri/src/reklamacije.rs
git commit -m "$(cat <<'EOF'
feat(sw-7): add the cl. 63 st. 3 attestation columns and notice

v16 adds no_fee_attested / no_fee_attested_at to reklamacije, defaulting to
UNattested — a record predating the gate was never asked, and 0 must not read
as compliance. get_reklamacija derives no_fee_notice from the record's own
frozen regime, so every renderer gates on Option presence and none re-derives
the regime. Old-regime records carry None: 88/2021 cl. 55 st. 3 has no fee ban.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: The attestation gate

**Files:**
- Modify: `src-tauri/src/reklamacije.rs` (`AnswerInput` at line 608; `log_answer` at line 694; `resolve_reklamacija` at line 868; test helper `answer` at line 1269; test literal at line 1297; test caller at line 1473)
- Modify: `src-tauri/src/commands/reklamacije.rs` (`reklamacija_resolve` at line 127)

**Interfaces:**
- Consumes: `NO_FEE_NOTICE`, the two columns and the three view fields from Task 1.
- Produces: `AnswerInput.no_fee_attested: bool`; `resolve_reklamacija(conn, id, nacin, event_date, no_fee_attested, acting, now)`; Tauri command `reklamacija_resolve(state, id, nacin, event_date, no_fee_attested)` — reached from JS as `{ id, nacin, eventDate, noFeeAttested }`.

- [ ] **Step 1: Write the failing gate tests**

Add to the `mod tests` block in `src-tauri/src/reklamacije.rs`:

```rust
    /// A NEW-regime answer that already satisfies the čl. 63 st. 10 express
    /// warning, so the ONLY thing a rejection can be about is the čl. 63 st. 3
    /// attestation. The bare `answer()` helper leaves the warning fields `None`
    /// and would be refused by the older gate first, testing nothing new.
    fn new_regime_answer(event_date: &str, no_fee_attested: bool) -> AnswerInput {
        AnswerInput {
            warning_duty: Some("Dužni ste da se izjasnite o predlogu.".into()),
            warning_consequences: Some("U suprotnom se smatra da ste odustali.".into()),
            warning_zastoj: Some("Rok za rešavanje ne teče do vašeg izjašnjenja.".into()),
            no_fee_attested,
            ..answer(event_date)
        }
    }

    #[test]
    fn new_regime_answer_requires_the_no_fee_attestation() {
        with_reklamacija_db("rek_no_fee_answer_gate", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();

            let unattested = new_regime_answer("2026-09-03T00:00:00Z", false);
            let error = log_answer(conn, created.id, &unattested, 1, "2026-09-03T08:00:00Z")
                .expect_err("an unattested new-regime answer must be refused");
            assert_eq!(error.code(), "validation_error");

            // A refused transition persists nothing.
            let untouched = get_reklamacija(conn, created.id, "2026-09-03T00:00:00Z").unwrap();
            assert!(untouched.events.is_empty(), "no event may be appended");
            assert!(!untouched.no_fee_attested);

            let attested = new_regime_answer("2026-09-03T00:00:00Z", true);
            let ok = log_answer(conn, created.id, &attested, 1, "2026-09-03T08:00:00Z")
                .expect("an attested answer is accepted");
            assert!(ok.no_fee_attested);
            assert_eq!(
                ok.no_fee_attested_at.as_deref(),
                Some("2026-09-03T08:00:00Z"),
                "the attestation is stamped from the caller's now, never datetime('now')"
            );
        });
    }

    #[test]
    fn new_regime_resolve_is_gated_even_without_an_answer() {
        with_reklamacija_db("rek_no_fee_resolve_gate", |conn| {
            // The memo §4(d) on-the-spot path: resolved without ever answering,
            // so an answer-only gate would let this record close unattested.
            let created = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();

            let error = resolve_reklamacija(
                conn,
                created.id,
                "Zamena",
                "2026-09-04T00:00:00Z",
                false,
                1,
                "2026-09-04T08:00:00Z",
            )
            .expect_err("an unattested new-regime resolve must be refused");
            assert_eq!(error.code(), "validation_error");

            let untouched = get_reklamacija(conn, created.id, "2026-09-04T00:00:00Z").unwrap();
            assert_eq!(untouched.status, "open", "a refused resolve must not close it");
            assert!(untouched.events.is_empty());

            let resolved = resolve_reklamacija(
                conn,
                created.id,
                "Zamena",
                "2026-09-04T00:00:00Z",
                true,
                1,
                "2026-09-04T08:00:00Z",
            )
            .expect("an attested resolve is accepted");
            assert_eq!(resolved.status, "resolved");
            assert!(resolved.no_fee_attested);
        });
    }

    #[test]
    fn the_attestation_is_asked_once_and_keeps_its_first_timestamp() {
        with_reklamacija_db("rek_no_fee_idempotent", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();

            let attested = new_regime_answer("2026-09-03T00:00:00Z", true);
            log_answer(conn, created.id, &attested, 1, "2026-09-03T08:00:00Z").unwrap();

            // Already attested at the answer: resolve must not ask again.
            let resolved = resolve_reklamacija(
                conn,
                created.id,
                "Zamena",
                "2026-09-05T00:00:00Z",
                false,
                1,
                "2026-09-05T08:00:00Z",
            )
            .expect("an already-attested record resolves without re-attesting");
            assert!(resolved.no_fee_attested);
            assert_eq!(
                resolved.no_fee_attested_at.as_deref(),
                Some("2026-09-03T08:00:00Z"),
                "the original attestation timestamp must survive"
            );
        });
    }

    #[test]
    fn the_old_regime_is_never_gated_on_the_fee_ban() {
        with_reklamacija_db("rek_no_fee_old_ungated", |conn| {
            // 88/2021 čl. 55 st. 3 has no fee ban (memo §6). Gating an old-regime
            // record on it would assert a duty that does not bind the shop.
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();

            // `answer()` leaves the warning fields None and attests nothing —
            // both are lawful under 88/2021, and neither may be demanded here.
            log_answer(conn, created.id, &answer("2026-06-05T00:00:00Z"), 1, "2026-06-05T08:00:00Z")
                .expect("an old-regime answer is never gated on the fee ban");

            let resolved = resolve_reklamacija(
                conn,
                created.id,
                "Popravka",
                "2026-06-10T00:00:00Z",
                false,
                1,
                "2026-06-10T08:00:00Z",
            )
            .expect("an old-regime resolve is never gated on the fee ban");
            assert_eq!(resolved.status, "resolved");
            assert!(
                !resolved.no_fee_attested,
                "the flag must stay unset — it is not a compliance signal here"
            );
            assert!(resolved.no_fee_attested_at.is_none());
        });
    }
```

- [ ] **Step 2: Run them and confirm they fail for the right reason**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib no_fee -- --test-threads=1`
Expected: FAIL to compile — `struct 'AnswerInput' has no field named 'no_fee_attested'` and `this function takes 6 arguments but 7 arguments were supplied`.

- [ ] **Step 3: Add the message const**

In `src-tauri/src/reklamacije.rs`, next to `MSG_NEW_ANSWER_WARNING`:

```rust
/// The NEW-regime attestation refusal (čl. 63 st. 3). Phrased as a confirmation
/// the operator must give, not as an accusation that a fee was charged.
const MSG_NEW_NO_FEE: &str = "Za novu reklamaciju potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).";
```

- [ ] **Step 4: Add the field to `AnswerInput`**

In `AnswerInput` (line 608), after `pub warning_zastoj: Option<String>,`:

```rust
    /// čl. 63 st. 3 attestation. Ignored under the OLD regime, which has no ban.
    pub no_fee_attested: bool,
```

- [ ] **Step 5: Write the gate helper**

In `src-tauri/src/reklamacije.rs`, next to `load_regime` (line 626):

```rust
/// 35/2026 čl. 63 st. 3. NEW regime only: 88/2021 čl. 55 st. 3 has no fee ban,
/// so gating an old-regime record on it would assert a duty that does not bind.
///
/// Idempotent by design — the operator confirms once, and every later transition
/// only reads the stored flag. Runs inside the caller's transaction and before
/// any event is appended, so a refusal persists nothing.
fn ensure_no_fee_attested(
    tx: &Connection,
    id: i64,
    regime: &str,
    attested_now: bool,
    now: &str,
) -> Result<(), AppError> {
    if regime != REGIME_NEW {
        return Ok(());
    }

    let already: i64 = tx.query_row(
        "SELECT no_fee_attested FROM reklamacije WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    if already != 0 {
        return Ok(());
    }

    if !attested_now {
        return Err(AppError::validation(
            MSG_NEW_NO_FEE,
            serde_json::json!({ "field": "noFeeAttested" }),
        ));
    }

    tx.execute(
        "UPDATE reklamacije SET no_fee_attested = 1, no_fee_attested_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}
```

- [ ] **Step 6: Wire it into `log_answer`**

In `log_answer` (line 694), immediately after the closing brace of the `if regime == REGIME_NEW { ... }` express-warning block and before the `let detail = ...` line:

```rust
    ensure_no_fee_attested(&tx, id, &regime, input.no_fee_attested, now)?;
```

**Order matters.** It must come *after* the express-warning check, not before. The existing test `new_regime_answer_requires_the_express_warning` (line 1280) feeds a `bare` answer that is missing both the warning and the attestation; putting the fee gate first would make that test pass for the wrong reason and stop covering čl. 63 st. 10.

- [ ] **Step 7: Wire it into `resolve_reklamacija`**

In `resolve_reklamacija` (line 868), change the signature and bind the regime:

```rust
pub fn resolve_reklamacija(
    conn: &mut Connection,
    id: i64,
    nacin: &str,
    event_date: &str,
    no_fee_attested: bool,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    require_non_empty(nacin, "nacin")?;
    parse_rfc3339(event_date, "eventDate")?;
    let tx = conn.transaction()?;
    let regime = load_regime(&tx, id)?;
    ensure_no_fee_attested(&tx, id, &regime, no_fee_attested, now)?;
```

The rest of the function body is unchanged.

- [ ] **Step 8: Update the command layer**

In `src-tauri/src/commands/reklamacije.rs`, `reklamacija_resolve` (line 127):

```rust
#[tauri::command]
pub fn reklamacija_resolve(
    state: State<'_, AppState>,
    id: i64,
    nacin: String,
    event_date: String,
    no_fee_attested: bool,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::resolve_reklamacija(
        &mut connection,
        id,
        &nacin,
        &event_date,
        no_fee_attested,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}
```

- [ ] **Step 9: Update the existing call sites the signature change breaks**

There are exactly three, and they do **not** all need the same value.

**(a) The `answer` helper, line 1269** — add the field as `false`. This helper is the *bare* fixture: its warning fields are already `None`, and it is used both to prove the express-warning gate rejects and to answer OLD-regime records, which are never fee-gated. `false` keeps it bare in both respects.

```rust
    fn answer(event_date: &str) -> AnswerInput {
        AnswerInput {
            answer_text: "Predlog zamene robe.".into(),
            warning_duty: None,
            warning_consequences: None,
            warning_zastoj: None,
            event_date: event_date.into(),
            no_fee_attested: false,
        }
    }
```

**(b) The `full` literal at line 1297** — this one is a NEW-regime answer asserted to be **accepted**, and it builds on `..answer(...)`, so it would inherit `no_fee_attested: false` and start failing the new gate. Add the field explicitly:

```rust
            let full = AnswerInput {
                warning_duty: Some("Dužni ste da se izjasnite o predlogu.".into()),
                warning_consequences: Some("U suprotnom se smatra da ste odustali.".into()),
                warning_zastoj: Some("Rok za rešavanje ne teče do vašeg izjašnjenja.".into()),
                no_fee_attested: true,
                ..answer("2026-09-05T00:00:00Z")
            };
```

**(c) The `resolve_reklamacija` caller at line 1473** (`resolve_marks_the_record_resolved`) — insert the argument between `event_date` and `acting`. That record is filed 2026-06-01, so it is OLD-regime and ungated; `false` is correct and proves it:

```rust
            let view = resolve_reklamacija(
                conn,
                created.id,
                "Zamena robe",
                "2026-06-05T00:00:00Z",
                false,
                1,
                "2026-06-05T09:00:00Z",
            )
            .unwrap();
```

The `&answer(...)` call sites at lines 1323 and 1426 need no change — both are OLD-regime records (filed 2026-06-01), where the gate short-circuits before it reads anything.

- [ ] **Step 10: Run the tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reklamacije -- --test-threads=1`
Expected: PASS. If `old_regime_full_cycle_restarts_to_the_memo_date` fails, its `resolve_reklamacija` call is missing the new argument.

- [ ] **Step 11: Format, lint, commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
git add src-tauri/src/reklamacije.rs src-tauri/src/commands/reklamacije.rs
git commit -m "$(cat <<'EOF'
feat(sw-7): gate the new-regime answer and resolve on the cl. 63 st. 3 attestation

ensure_no_fee_attested runs inside the caller's transaction and before any event
is appended, so a refusal persists nothing. Both log_answer and
resolve_reklamacija call it: the answer is where the determination is recorded,
and resolve is the backstop, because resolve_reklamacija requires no prior
answer (the memo §4(d) on-the-spot path) and would otherwise be escapable.

Idempotent — the operator confirms once and the second gate only reads the flag,
keeping the original timestamp. The old regime short-circuits before the read:
88/2021 cl. 55 st. 3 has no fee ban, so its flag stays unset and that unset is
not a compliance signal.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Widen the `reklamacija_breach` summary

**Files:**
- Modify: `src-tauri/src/legal.rs` (`reklamacija_breach` summary, near line 175; tests in `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: no signature change — copy only.

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `src-tauri/src/legal.rs`:

```rust
    #[test]
    fn reklamacija_breach_summary_is_not_narrowed_to_deadlines() {
        // čl. 210 st. 1 tač. 24 penalises breach of čl. 63 st. 3 — the fee ban —
        // alongside the deadline stavovi, so a summary that says only "rokovi"
        // understates what the figure beside it is the sanction for.
        for regime in [ReklamacijaRegime::Old, ReklamacijaRegime::New] {
            let notice = reklamacija_breach(&profile(Some(PravnaForma::Preduzetnik)), regime);
            assert!(
                !notice.summary.contains("rokovima"),
                "the summary must cover the duty set, not just the clock: {}",
                notice.summary
            );
            // …but it must stay regime-neutral: the same sentence renders on an
            // OLD-regime record, which carries no fee ban at all.
            assert!(
                !notice.summary.contains("naplat"),
                "the NEW-only fee ban must not be named in shared copy: {}",
                notice.summary
            );
        }
    }
```

- [ ] **Step 2: Run it and confirm it fails for the right reason**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reklamacija_breach_summary -- --test-threads=1`
Expected: FAIL — `the summary must cover the duty set, not just the clock: Nepostupanje po reklamaciji potrošača u propisanim rokovima je prekršaj.`

- [ ] **Step 3: Widen the summary**

In `reklamacija_breach`, replace the summary line:

```rust
        summary: "Nepostupanje po propisanim obavezama u vezi sa reklamacijom potrošača \
                  je prekršaj."
            .to_string(),
```

- [ ] **Step 4: Run the legal tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legal:: -- --test-threads=1`
Expected: PASS, all legal tests green.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
git add src-tauri/src/legal.rs
git commit -m "$(cat <<'EOF'
fix(sw-7): widen the reklamacija_breach summary beyond deadlines

cl. 210 st. 1 tac. 24 penalises breach of cl. 63 st. 3 — the fee ban — alongside
the deadline stavovi, so "u propisanim rokovima" understated what the figure
beside it sanctions. The replacement stays regime-neutral: the same sentence
renders on old-regime records, which carry no fee ban, so it names the duty set
without naming the new-only ban.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: The two printed documents

**Files:**
- Modify: `src-tauri/src/reklamacije_docs.rs` (`render_potvrda_html` at line 46; `render_notice_html` at line 121; tests in `mod tests` at line 171)

**Interfaces:**
- Consumes: `ReklamacijaView.no_fee_notice` from Task 1.
- Produces: no signature change.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src-tauri/src/reklamacije_docs.rs`:

```rust
    #[test]
    fn potvrda_carries_the_fee_ban_only_for_a_new_regime_record() {
        with_reklamacija_db("docs_potvrda_no_fee", |conn| {
            let new = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z", "Petar Petrović"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();
            let html = render_potvrda_html(&new);
            assert!(
                html.contains("Zabranjeno je naplatiti utvrđivanje nesaobraznosti"),
                "a new-regime potvrda must carry čl. 63 st. 3"
            );

            // A pre-cutover complaint is governed by 88/2021 čl. 55 st. 3, which
            // has no fee ban. Printing one on its potvrda tells the consumer they
            // have a right the law does not give them for this complaint.
            let old = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z", "Marija Marić"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            let html = render_potvrda_html(&old);
            assert!(
                !html.contains("Zabranjeno je naplatiti"),
                "an old-regime potvrda must not assert the fee ban"
            );
        });
    }

    #[test]
    fn prodajno_mesto_notice_always_states_the_fee_ban() {
        // The display notice is static and forward-looking — every complaint
        // lodged from today on is new-regime — so it states current law.
        let html = render_notice_html();
        assert!(html.contains("Zabranjeno je naplatiti utvrđivanje nesaobraznosti"));
        assert!(html.contains("čl. 63 st. 3"));
    }
```

- [ ] **Step 2: Run them and confirm they fail for the right reason**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reklamacije_docs -- --test-threads=1`
Expected: FAIL — `a new-regime potvrda must carry čl. 63 st. 3` and the notice assertion. The OLD-regime assertion should already pass; if it fails, something is rendering the ban unconditionally.

- [ ] **Step 3: Add the gated paragraph to the potvrda**

In `render_potvrda_html`, after `html.push_str("</table>\n");` and before the `<footer>`:

```rust
    // NEW regime only — gated on the Option, never on a regime test of this
    // renderer's own. An old-regime potvrda must not promise a right that
    // 88/2021 čl. 55 st. 3 does not give.
    if let Some(no_fee) = view.no_fee_notice.as_deref() {
        html.push_str(&format!(
            "<p class=\"meta\">{}</p>\n",
            escape_html(no_fee)
        ));
    }
```

- [ ] **Step 4: Add the sentence to the prodajno-mesto notice**

In `render_notice_html`, after the 8-day answer paragraph and before the packaging paragraph:

```rust
    html.push_str(
        "<p>Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). Otklanjanje \
         nesaobraznosti — popravka ili zamena — je bez naknade (čl. 56 st. 1).</p>\n",
    );
```

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reklamacije_docs -- --test-threads=1`
Expected: PASS, all document tests green including the pre-existing escaping tests.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
git add src-tauri/src/reklamacije_docs.rs
git commit -m "$(cat <<'EOF'
feat(sw-7): print the cl. 63 st. 3 fee ban on the potvrda and the display notice

The potvrda is per-record and gates on no_fee_notice, so a pre-cutover complaint
does not promise the consumer a right 88/2021 cl. 55 st. 3 does not give for it.
The prodajno-mesto notice is static and forward-looking, so it states current law
unconditionally.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Services layer

**Files:**
- Modify: `src/services/types.ts` (`ReklamacijaView` at line 672; `AnswerInput` near line 660)
- Modify: `src/services/ports.ts` (`ReklamacijeService.resolve` at line 256)
- Modify: `src/services/local-adapter.ts` (`resolve` at line 280)
- Modify: `src/services/mock-adapter.ts` (`mockReklamacijaView`, `logAnswer`, `resolve`)
- Modify: `src/services/local-adapter.test.ts` (lines 759 and 1303 construct `AnswerInput`; the resolve call at line 776 and its `toHaveBeenNthCalledWith(8, …)` assertion at line 802)

**Interfaces:**
- Consumes: the Rust contract from Tasks 1–2.
- Produces: `ReklamacijaView.noFeeAttested` / `noFeeAttestedAt` / `noFeeNotice`; `AnswerInput.noFeeAttested`; `ReklamacijeService.resolve(id, nacin, eventDate, noFeeAttested)`.

- [ ] **Step 1: Write the failing adapter test**

In `src/services/local-adapter.test.ts`, change the resolve call in the reklamacije mapping test (line 776) to pass the flag, and its assertion (line 802) to expect it:

```ts
    await services.reklamacije.resolve(7, "zamena", "2026-06-25T00:00:00Z", true);
```

and the matching expectation:

```ts
    expect(invoke).toHaveBeenNthCalledWith(8, "reklamacija_resolve", {
      id: 7,
      nacin: "zamena",
      eventDate: "2026-06-25T00:00:00Z",
      noFeeAttested: true,
    });
```

(Keep the index at `8` — do not renumber it or the assertions after it; only this payload changes.)

- [ ] **Step 2: Run it and confirm it fails for the right reason**

Run: `bun run test -- src/services/local-adapter.test.ts`
Expected: FAIL — the received call lacks `noFeeAttested`.

- [ ] **Step 3: Extend the types**

In `src/services/types.ts`, `ReklamacijaView`, after `notice: LegalNotice;`:

```ts
  /** čl. 63 st. 3 attestation. Always `false` under the OLD regime, which has
   *  no fee ban — that `false` is not a compliance signal. */
  noFeeAttested: boolean;
  noFeeAttestedAt: string | null;
  /** The prohibition text, `null` unless this record's frozen regime is NEW.
   *  Render on presence; never re-derive the regime in a component. */
  noFeeNotice: string | null;
```

In `AnswerInput`, after `warningZastoj: string | null;`:

```ts
  /** čl. 63 st. 3 attestation. Ignored by the backend under the OLD regime. */
  noFeeAttested: boolean;
```

- [ ] **Step 4: Extend the port and the Tauri adapter**

`src/services/ports.ts`:

```ts
  resolve(
    id: number,
    nacin: string,
    eventDate: string,
    noFeeAttested: boolean,
  ): Promise<ReklamacijaView>;
```

`src/services/local-adapter.ts`:

```ts
      resolve: (id, nacin, eventDate, noFeeAttested) =>
        invoke<ReklamacijaView>("reklamacija_resolve", {
          id,
          nacin,
          eventDate,
          noFeeAttested,
        }),
```

- [ ] **Step 5: Run the adapter test and confirm it passes**

Run: `bun run test -- src/services/local-adapter.test.ts`
Expected: PASS. Fix the two `AnswerInput` literals (lines 759 and 1303) by adding `noFeeAttested: true,` if TypeScript complains during `bun run build` later.

- [ ] **Step 6: Write the failing mock-adapter test**

Create `src/services/mock-adapter.reklamacije.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { createMockServices } from "./mock-adapter";
import type { ReklamacijaInput } from "./types";

function intake(filedAt: string): ReklamacijaInput {
  return {
    podnosilacImePrezime: "Petar Petrović",
    kontakt: null,
    podaciORobi: "Veš mašina Beko",
    opisNesaobraznosti: "Ne centrifugira",
    zahtev: "Popravka",
    robaKind: "tehnicka",
    filedAt,
  };
}

describe("mock reklamacije — čl. 63 st. 3", () => {
  it("refuses an unattested new-regime resolve, like the backend does", async () => {
    const services = createMockServices();
    const view = await services.reklamacije.create(intake("2026-09-01T00:00:00Z"));

    expect(view.noFeeNotice).toContain("utvrđivanje nesaobraznosti");

    // A double that accepts what the backend rejects is worse than no double.
    await expect(
      services.reklamacije.resolve(view.id, "Zamena", "2026-09-04T00:00:00Z", false),
    ).rejects.toMatchObject({ code: "validation_error" });

    const resolved = await services.reklamacije.resolve(
      view.id,
      "Zamena",
      "2026-09-04T00:00:00Z",
      true,
    );
    expect(resolved.status).toBe("resolved");
    expect(resolved.noFeeAttested).toBe(true);
  });

  it("never gates an old-regime record", async () => {
    const services = createMockServices();
    const view = await services.reklamacije.create(intake("2026-06-01T00:00:00Z"));

    expect(view.noFeeNotice).toBeNull();

    const resolved = await services.reklamacije.resolve(
      view.id,
      "Popravka",
      "2026-06-10T00:00:00Z",
      false,
    );
    expect(resolved.status).toBe("resolved");
    expect(resolved.noFeeAttested).toBe(false);
  });
});
```

- [ ] **Step 7: Run it and confirm it fails for the right reason**

Run: `bun run test -- src/services/mock-adapter.reklamacije.test.ts`
Expected: FAIL — `expected undefined to contain 'utvrđivanje nesaobraznosti'`, because `mockReklamacijaView` does not yet produce `noFeeNotice`.

- [ ] **Step 8: Mirror the gate in the mock adapter**

In `src/services/mock-adapter.ts`, beside `reklamacijaNotice`:

```ts
/** `reklamacije.rs::NO_FEE_NOTICE` verbatim. NEW regime only. */
const REKLAMACIJA_NO_FEE_NOTICE =
  "Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). " +
  "Otklanjanje nesaobraznosti — popravka ili zamena — je bez naknade po " +
  "posebnoj odredbi (čl. 56 st. 1), i u starom i u novom režimu.";

/** Mirrors `reklamacije.rs::ensure_no_fee_attested`. A double that accepts what
 *  the backend rejects hides the gate from every UI test that uses it. */
function assertNoFeeAttested(view: ReklamacijaView, attested: boolean): void {
  if (view.noFeeNotice === null || view.noFeeAttested) {
    return;
  }
  if (!attested) {
    throw {
      code: "validation_error",
      message:
        "Za novu reklamaciju potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).",
    };
  }
  view.noFeeAttested = true;
  view.noFeeAttestedAt = now;
}
```

In `mockReklamacijaView`, after `notice: reklamacijaNotice(regime),`:

```ts
    noFeeAttested: false,
    noFeeAttestedAt: null,
    noFeeNotice: regime === "new" ? REKLAMACIJA_NO_FEE_NOTICE : null,
```

In the `logAnswer` method, as the first statement after `const view = findReklamacija(reklamacije, id);`:

```ts
        assertNoFeeAttested(view, input.noFeeAttested);
```

Change the `resolve` method's signature and add the same guard:

```ts
      async resolve(id, nacin, eventDate, noFeeAttested) {
        const view = findReklamacija(reklamacije, id);
        assertNoFeeAttested(view, noFeeAttested);
        view.events.push({
          eventType: "resolved",
          eventDate,
          detailJson: JSON.stringify({ nacin }),
          consumerConsent: false,
        });
        view.status = "resolved";
        recomputeMockDeadlines(view);
        return view;
      },
```

- [ ] **Step 9: Run the tests and confirm they pass**

Run: `bun run test -- src/services/`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
bun run build
git add src/services/
git commit -m "$(cat <<'EOF'
feat(sw-7): carry the cl. 63 st. 3 attestation through the services layer

ReklamacijaView gains noFeeAttested / noFeeAttestedAt / noFeeNotice, AnswerInput
gains noFeeAttested, and resolve takes the flag. The mock adapter mirrors the
backend gate rather than waving it through: a double that accepts what the
backend rejects hides the gate from every UI test that uses it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Module UI

**Files:**
- Modify: `src/app/reklamacije/ReklamacijeModule.tsx` (`DetailPanel` at line 347, its record-fields `</dl>` at line 442; `AnswerForm` at line 620; `ResolveForm` at line 923)
- Modify: `src/app/reklamacije/ReklamacijeModule.test.tsx` (fixtures near line 80; detail describe block)

**Interfaces:**
- Consumes: `noFeeNotice`, `noFeeAttested` on `ReklamacijaView`; `noFeeAttested` on `AnswerInput`; `resolve(id, nacin, eventDate, noFeeAttested)`.
- Produces: nothing downstream.

- [ ] **Step 1: Add the fixture fields and write the failing tests**

In `ReklamacijeModule.test.tsx`, add to the `newRegimeOpen` fixture (and therefore to `oldRegimeOpen`, `extensionUsed`, `overdue` which spread it) and to `createdView`:

```ts
  noFeeAttested: false,
  noFeeAttestedAt: null,
  noFeeNotice:
    "Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). " +
    "Otklanjanje nesaobraznosti — popravka ili zamena — je bez naknade po " +
    "posebnoj odredbi (čl. 56 st. 1), i u starom i u novom režimu.",
```

`oldRegimeOpen` must override it — the OLD regime carries no ban:

```ts
const oldRegimeOpen: ReklamacijaView = {
  ...newRegimeOpen,
  id: 11,
  registerNumber: 11,
  regime: "old",
  filedAt: "2026-06-01T00:00:00Z",
  podnosilacImePrezime: "Marija Marić",
  noFeeNotice: null,
};
```

Then add to the `ReklamacijeModule detail` describe block:

```tsx
  it("requires the no-fee attestation before a new-regime answer", async () => {
    const user = userEvent.setup();
    const services = servicesWithView(newRegimeOpen);
    const logAnswer = vi.spyOn(services.reklamacije, "logAnswer");

    render(<ReklamacijeModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za reklamaciju #10" }),
    );

    // The standing prohibition is visible without any action.
    expect(
      await screen.findByText(/Zabranjeno je naplatiti utvrđivanje nesaobraznosti/),
    ).toBeInTheDocument();

    await user.type(
      await screen.findByLabelText("Odgovor na reklamaciju"),
      "Prihvatamo reklamaciju.",
    );
    await user.click(screen.getByRole("button", { name: "Unesi odgovor" }));

    expect(
      await screen.findByText(/Potvrdite da utvrđivanje nesaobraznosti nije naplaćeno/),
    ).toBeInTheDocument();
    expect(logAnswer).not.toHaveBeenCalled();

    await user.click(
      screen.getByLabelText("Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)"),
    );
    await user.click(screen.getByRole("button", { name: "Unesi odgovor" }));

    await waitFor(() => expect(logAnswer).toHaveBeenCalled());
    expect(logAnswer.mock.calls[0]?.[1]).toMatchObject({ noFeeAttested: true });
  });

  it("shows no fee-ban copy or checkbox for an old-regime record", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWithView(oldRegimeOpen)} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za reklamaciju #11" }),
    );

    // 88/2021 čl. 55 st. 3 has no fee ban — asserting one here would state a
    // duty that does not bind this complaint.
    expect(await screen.findByText("Stari režim")).toBeInTheDocument();
    expect(screen.queryByText(/Zabranjeno je naplatiti/)).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText(
        "Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)",
      ),
    ).not.toBeInTheDocument();
  });

  it("drops the checkbox once the record is already attested", async () => {
    const user = userEvent.setup();
    render(
      <ReklamacijeModule
        services={servicesWithView({
          ...newRegimeOpen,
          noFeeAttested: true,
          noFeeAttestedAt: "2026-09-03T08:00:00Z",
        })}
      />,
    );

    await user.click(
      await screen.findByRole("button", { name: "Detalji za reklamaciju #10" }),
    );

    // Asked once, not once per transition — but the duty stays on screen.
    expect(
      await screen.findByText(/Zabranjeno je naplatiti utvrđivanje nesaobraznosti/),
    ).toBeInTheDocument();
    expect(
      screen.queryByLabelText(
        "Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)",
      ),
    ).not.toBeInTheDocument();
  });
```

- [ ] **Step 2: Run them and confirm they fail for the right reason**

Run: `bun run test -- src/app/reklamacije/ReklamacijeModule.test.tsx`
Expected: FAIL — `Unable to find an element with the text: /Zabranjeno je naplatiti…/`. The old-regime test should already pass (nothing renders it yet); that is fine.

- [ ] **Step 3: Render the standing prohibition**

In `DetailPanel`, immediately after the closing `</dl>` of the record-fields grid (line 442) and before the `<Separator />` that precedes „Rokovi". Note there are two `</dl>` tags in this component — 442 is the record fields, 483 is the Rokovi grid; use the first:

```tsx
      {view.noFeeNotice ? (
        // A standing duty for the whole life of a new-regime complaint, so it is
        // NOT gated on `overdue` the way the deadline advisory is. `null` under
        // the old regime, which carries no fee ban — the gating is the backend's.
        <p className="text-xs text-muted-foreground">{view.noFeeNotice}</p>
      ) : null}
```

- [ ] **Step 4: Add the checkbox to `AnswerForm`**

In `AnswerForm`, after the existing `const isNew = view.regime === "new";` line:

```tsx
  const noFeeRequired = view.noFeeNotice !== null && !view.noFeeAttested;
  const [noFee, setNoFee] = useState(false);
```

In `handleSubmit`, after the express-warning check and before `setSubmitting(true)`:

```tsx
    if (noFeeRequired && !noFee) {
      setError("Potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).");
      return;
    }
```

Add `noFeeAttested: noFee,` to the `service.logAnswer` payload, after `eventDate`.

Render the field inside the `<FieldGroup>`, after the date `<Field>`:

```tsx
        {noFeeRequired ? (
          <Field orientation="horizontal">
            <Checkbox
              id="reklamacija-bez-naplate"
              checked={noFee}
              onCheckedChange={(checked) => setNoFee(Boolean(checked))}
            />
            <FieldLabel htmlFor="reklamacija-bez-naplate">
              Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)
            </FieldLabel>
          </Field>
        ) : null}
```

- [ ] **Step 5: Add the same checkbox to `ResolveForm`**

`ResolveForm` currently destructures `{ view, service, runAction }`. Add at the top of the component:

```tsx
  const noFeeRequired = view.noFeeNotice !== null && !view.noFeeAttested;
  const [noFee, setNoFee] = useState(false);
```

In its `handleSubmit`, after the `nacin` check:

```tsx
    if (noFeeRequired && !noFee) {
      setError("Potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).");
      return;
    }
```

Change the service call to pass the flag:

```tsx
      () => service.resolve(view.id, nacin.trim(), toRfc3339(datum), noFee),
```

And render the field inside its `<FieldGroup>`, after the date `<Field>` — the same block as Step 4 but with `id="reklamacija-resenje-bez-naplate"` and `htmlFor` matching:

```tsx
        {noFeeRequired ? (
          <Field orientation="horizontal">
            <Checkbox
              id="reklamacija-resenje-bez-naplate"
              checked={noFee}
              onCheckedChange={(checked) => setNoFee(Boolean(checked))}
            />
            <FieldLabel htmlFor="reklamacija-resenje-bez-naplate">
              Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)
            </FieldLabel>
          </Field>
        ) : null}
```

Note: both forms can be on screen at once, so the two ids must differ. The test in Step 1 targets the answer form's label; if `getByLabelText` reports multiple matches, the ids were not made distinct.

- [ ] **Step 6: Run the tests and confirm they pass**

Run: `bun run test -- src/app/reklamacije/ReklamacijeModule.test.tsx`
Expected: PASS, all tests in the file green.

- [ ] **Step 7: Run every gate**

```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Expected: all pass. Do not proceed to the commit until every one is green.

- [ ] **Step 8: Commit**

```bash
git add src/app/reklamacije/
git commit -m "$(cat <<'EOF'
feat(sw-7): surface the cl. 63 st. 3 ban and its attestation in the module

The prohibition is a standing duty for the life of a new-regime complaint, so it
renders unconditionally on presence of noFeeNotice rather than being gated on
overdue the way the deadline advisory is. The attestation checkbox appears on
both the answer and resolve forms until the record is attested, then disappears
— asked once, not once per transition. Old-regime records show neither: 88/2021
cl. 55 st. 3 has no fee ban.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Close the compliance-register residual

**Files:**
- Modify: `docs/SERBIAN-LAW-COMPLIANCE.md` (row 23 status cell, line 78; SW-7 roadmap row, line 111)
- Modify: `docs/PROGRESS.md` (Compliance Backlog Update table if a reklamacija row is present)

**Interfaces:**
- Consumes: the shipped behaviour from Tasks 1–6.
- Produces: nothing.

- [ ] **Step 1: Re-state row 23**

Row 23's status cell currently opens `**Clock + warning shipped 31.07.2026 (SW-7); no-fee ban NOT shipped**` and ends with a `**Still open:**` clause about čl. 63 st. 3. Replace that opening with `**Shipped (SW-7)**` and replace the `**Still open:**` clause with a description of what now exists: the `no_fee_attested` column, the `ensure_no_fee_attested` gate on both `log_answer` and `resolve_reklamacija`, the standing prohibition, and the potvrda/notice copy — stating explicitly that it is warn-and-attest, not a till-side block, and why.

Change the Required-action cell from `SW-7 (no-fee ban, NEW regime only)` to `SW-7`.

- [ ] **Step 2: Re-state the SW-7 roadmap row**

Remove "Residual 1" (the no-fee ban) from the roadmap row's residual list and renumber the remaining one — the register is admin-gated but reads are not access-logged, which folds into SW-10. Keep that residual; this plan does not touch it.

- [ ] **Step 3: Verify the docs are internally consistent**

Run:

```bash
grep -n "no-fee ban NOT shipped\|Residual 1" docs/SERBIAN-LAW-COMPLIANCE.md
```

Expected: no output. Any hit is a stale claim that now contradicts the code.

Then confirm the markdown tables did not lose a column:

```bash
awk 'NR>=54 && NR<=91 {n=gsub(/\|/,"|"); if (n!=7 && n!=0) print "LINE "NR" has "n" pipes"}' docs/SERBIAN-LAW-COMPLIANCE.md
```

Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add docs/
git commit -m "$(cat <<'EOF'
docs(sw-7): close the cl. 63 st. 3 residual in the compliance register

Row 23 and the SW-7 roadmap row named the no-fee ban as not shipped. It is now
implemented as warn-and-attest — the register has no money path to block, which
the row states rather than implying a stronger guarantee than the code provides.

The access-logging residual on row 22 stands and is untouched.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| §1 Schema v16 | 1 |
| §2 Gate (`ensure_no_fee_attested`, both call sites, `MSG_NEW_NO_FEE`) | 2 |
| §3 Copy + `no_fee_notice: Option<String>` | 1 (const + derivation), 6 (render) |
| §4 `legal.rs` summary correction | 3 |
| §5 Surfaces — module | 6 |
| §5 Surfaces — potvrda + notice | 4 |
| §5 Surfaces — services layer | 5 |
| §6 Tests | folded into each task |
| Residual bookkeeping | 7 |

**Placeholder scan:** clean — every code step carries the actual code, every run step the actual command and expected output.

**Type consistency:** `no_fee_attested` / `no_fee_attested_at` / `no_fee_notice` in Rust map to `noFeeAttested` / `noFeeAttestedAt` / `noFeeNotice` in TypeScript throughout. `ensure_no_fee_attested(tx, id, regime, attested_now, now)` is defined in Task 2 Step 5 and called in Steps 6 and 7 with that argument order. `resolve_reklamacija`'s new parameter sits between `event_date` and `acting` in Task 2 Steps 7, 8 and 9 and in the Task 5 adapter payload.

**Known ordering constraint:** Task 2 changes `resolve_reklamacija`'s arity, which breaks `commands/reklamacije.rs` and one test call site in the same commit, and adds a field to `AnswerInput`, which breaks one test helper and one struct literal. Step 9 lists all four by line, with the correct value for each. Do not split Task 2 across commits.

**Two traps found while checking the plan against the code, both already handled above:**

1. `answer()` leaves the three warning fields `None`, so a NEW-regime test built on it would be refused by the čl. 63 st. 10 express-warning gate and never reach the fee gate — passing for the wrong reason. Task 2 Step 1 therefore introduces `new_regime_answer(event_date, no_fee_attested)`, which supplies the warning and varies only the attestation.
2. The `full` literal at line 1297 uses struct-update syntax over `answer(...)`, so it silently inherits whatever default the helper gets. Left alone it would inherit `false` and turn a passing "accepted" assertion into a failure. Task 2 Step 9(b) sets it explicitly.

**Assertion style:** this module asserts `error.code() == "validation_error"`, not `matches!(error, AppError::Validation { .. })`. The plan follows the module.
