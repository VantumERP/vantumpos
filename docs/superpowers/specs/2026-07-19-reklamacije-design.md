# Reklamacije Register + Deadline Engine (SW-7) — Design

**Date:** 2026-07-19
**Status:** Approved (full module in one cycle; three engineering calls confirmed)
**Legal authority:** `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md` (adversarially verified 19.07.2026; 31/32 load-bearing rules survived). That memo is the authority for every deadline, field, and text below — this spec encodes it, it does not restate law.
**Builds on:** SW-6c HTML-renderer pattern (`escape_html`/`format_*`), SW-8 print primitive (`PrintService.openForPrint`), the `take_next_receipt_number` sequential-allocation pattern, `require_admin` role-gating.

## Scope

The full consumer-complaint module in one cycle: the register (11 verified evidencija fields), a **regime-versioned deadline engine**, intake with a synchronous sequential register number + printable potvrda o prijemu, the mandatory express warning (new regime) and prodajno-mesto notice, 2-year retention gating, and PII role-gating.

## Confirmed engineering calls

| Call | Decision |
|---|---|
| Deadline model | **Store raw dated events; derive deadlines** with a pure regime-branched function on every read (the `compute_prethodna_cena` shape). A stored due-date can drift from the events; a derived one cannot. |
| Lifecycle event dates | **Entered explicitly** (default today, editable). The suspend/resume legally triggers on real-world dates the shop must prove (memo §2.6 #3), not on "when I clicked". |
| Consumer PII | **Inline** on the reklamacija record. No customers table (YAGNI — the register is the only consumer-data need today). |
| Cutover date | A **configurable constant** `CUTOVER_DATE`, default `2026-08-01`, with a counsel-flag comment. Not a settings screen (the 1-vs-2-Aug question is unresolved; §1 of the memo). |
| PII access control | `require_admin` on the whole module + per-event `user_id` attribution. Read-access audit logging is SW-10 scope, deliberately not pulled in. |

## 1. Schema — migration v11

Count assertion 10 → 11. New tables join `CORE_TABLES`; indexes join `EXPLICIT_INDEXES`.

```sql
CREATE TABLE reklamacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    register_number INTEGER NOT NULL UNIQUE,          -- "broj pod kojim je zavedena" (čl. 55/63 st. 7)
    regime TEXT NOT NULL CHECK (regime IN ('old', 'new')),   -- persisted at creation; never recomputed
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'answered', 'awaiting_consumer', 'impasse', 'resolved')),
    filed_at TEXT NOT NULL,                            -- RFC3339; = prijem = podnošenje
    -- the 11 verified evidencija fields (čl. 55/63 st. 8), identical both regimes:
    podnosilac_ime_prezime TEXT NOT NULL,
    kontakt TEXT,                                      -- optional consumer contact (PII)
    podaci_o_robi TEXT NOT NULL,
    opis_nesaobraznosti TEXT NOT NULL,
    zahtev TEXT NOT NULL,                              -- consumer's requested remedy
    roba_kind TEXT NOT NULL DEFAULT 'opsta'
        CHECK (roba_kind IN ('opsta', 'tehnicka', 'namestaj')),   -- drives 15 vs 30 days
    datum_izdavanja_potvrde TEXT NOT NULL,            -- stamped at intake
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
    event_date TEXT NOT NULL,                          -- the real-world date (explicit, editable)
    detail_json TEXT,                                  -- answer text + warning fields; nacin rešavanja; agreed date
    consumer_consent INTEGER NOT NULL DEFAULT 0 CHECK (consumer_consent IN (0, 1)),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_reklamacija_events_parent ON reklamacija_events(reklamacija_id, event_date);
```

`register_number` allocated atomically inside the intake transaction as `MAX(register_number)+1` (single-instance desktop; UNIQUE guards it). The consumer PII fields (`podnosilac_ime_prezime`, `kontakt`) live here inline.

## 2. Deadline engine — pure function (`src-tauri/src/reklamacije.rs`)

`compute_deadlines(regime, filed_at, roba_kind, events, today) -> DeadlineState`, unit-tested to the day. All date math in Rust via `time`; calendar days; **never** a weekend/holiday rollover that extends a date (memo §2.6 #1).

```rust
pub struct DeadlineState {
    pub answer_due: String,                 // filed_at + 8 cal days (hard, never suspended, both regimes)
    pub resolution_due: Option<String>,     // None while the clock is paused (new) or at impasse
    pub clock: String,                      // "running" | "paused" | "restarted" | "impasse" | "resolved"
    pub consumer_window_due: Option<String>,// consumer_received_answer + 3 days, when applicable
    pub answer_overdue: bool,
    pub resolution_overdue: bool,
    pub one_extension_used: bool,
}
```

Rules (from memo §2, verbatim math):
- `answer_due = filed_at + 8`. `resolution_base = filed_at + (roba_kind ∈ {tehnicka,namestaj} ? 30 : 15)`.
- Layer an `extension_granted` (at most one) → base becomes the consented `event_date`.
- **OLD regime (restart):** if a `consumer_responded` event exists → `resolution_due = consumer_responded.event_date + (15|30)` (fresh full period, clock="restarted"). Else base/extended.
- **NEW regime (suspend/resume):** `suspension = consumer_responded.event_date − consumer_received_answer.event_date` when both exist → `resolution_due = base + suspension` (clock="running"). If `consumer_received_answer` exists but not `consumer_responded` → clock="paused", `resolution_due = None` (frozen). 
- **3-day window / silence:** when `consumer_received_answer` exists, `consumer_window_due = +3 days`; if `today > consumer_window_due` and no `consumer_responded` → clock="impasse", status `impasse`, `nije_saglasan` (both regimes; not new). Never auto-extend.
- **Extension × restart/resume interaction (safe default):** the extension (trader-can't-meet-deadline) and the restart/resume (answer→consumer-response cycle) are independent flows that can both occur. When both would produce a resolution date, take the **earlier (tighter)** of the two — under-promising a deadline is safe; over-promising is the harm (memo §2.6).
- Reminders fire ≥1 day early; a due date reached is a hard ceiling.

Status is derived from events too: `open` → `answered` (answer_given) → `awaiting_consumer` (consumer_received_answer) → `impasse` (silence) or back to running (consumer_responded) → `resolved` (resolved event).

## 3. Intake + potvrda o prijemu

`reklamacija_create` (`require_admin`, acting id from session), one transaction: compute `regime = filed_at < CUTOVER_DATE ? 'old' : 'new'`; allocate `register_number`; insert the record with all evidencija fields; stamp `datum_izdavanja_potvrde = now`. Then a **potvrda o prijemu** HTML document (`reklamacije_docs.rs`, SW-6c renderer shape) carries the register number, filing date, goods, and complaint — opened via the SW-8 `openForPrint`. „Bez odlaganja" is **not** a timer — no due-date computed for the potvrda (memo §3).

## 4. Lifecycle commands (all `require_admin`)

- `reklamacija_log_answer(id, answer_text, warning_fields, event_date)` — appends `answer_given`. **New-regime gate:** rejects unless all three express-warning elements are present (duty-to-respond / consequences / zastoj) — the memo's pre-filled Serbian template is editable but the fields must be non-empty. **Old-regime: no warning required** (rejecting for its absence would be wrong).
- `reklamacija_log_consumer_received_answer(id, event_date)` / `reklamacija_log_consumer_response(id, event_date)` — the suspend/resume (or restart) triggers.
- `reklamacija_grant_extension(id, new_deadline, consumer_consent, reason, event_date)` — one only (second blocked); requires `consumer_consent = true`.
- `reklamacija_resolve(id, nacin, event_date)` — appends `resolved`.
- `reklamacija_get(id)` / `reklamacija_list(filters)` — return the record + events + the computed `DeadlineState`.

## 5. Mandatory texts

- **Express 3-part warning (new regime only)** — pre-filled from the memo §4(a) template, editable, gated at `log_answer`.
- **Prodajno-mesto notice** — a printable „Obaveštenje o načinu i mestu prijema reklamacija" HTML document (memo §4(b)), opened via SW-8.
- No-fee for utvrđivanje nesaobraznosti — **new regime**: an advisory line; never presented as binding for old-regime complaints.

## 6. Retention + PII

- Retention is a purge-*eligibility* flag computed as `filed_at + 2y ≤ today` (a floor, not a "delete by" date — indefinite retention is compliant; memo §3). The module surfaces eligibility; it does not auto-delete.
- The whole module is `require_admin`-gated (consumer PII). Lawful basis is legal-obligation → **no consent UI**. Each event carries `user_id`.

## 7. Penalties + presumption (advisory only)

Regime-versioned penalty figures (preduzetnik 30.000 old / 100.000 new) shown as context on an overdue complaint — never a threat, never the 300k–2M tier. The nesaobraznost presumption (old 6mo/prelazak rizika; new 1yr/isporuka) is an optional advisory flag on intake, not a hard rule; deferred as a nicety unless trivial.

## 8. Frontend

`ReklamacijeService` on `PosServices`. A new admin-only „Reklamacije" nav tab: a list (register number, filer, status, the computed answer/resolution due-dates with overdue emphasis), an intake form (the evidencija fields + roba_kind + filing date), and a detail/timeline view showing the computed `DeadlineState`, the event history, and the lifecycle actions (log answer with the warning template for new-regime, log consumer-received/responded, grant extension, resolve) plus „Štampaj potvrdu" and „Štampaj obaveštenje" buttons (SW-8). Regime and its consequences are shown as read-only context.

## 9. Testing

The memo's two worked examples verbatim:
- **OLD/general/restart:** filed 2026-06-01 → answer_due 06-09, base resolution 06-16; consumer_received 06-06, consumer_responded 06-08 → resolution restarts to **06-23**.
- **NEW/tehnička/suspend:** filed 2026-08-15 → answer_due 08-23, base resolution 09-14; consumer_received 08-21, consumer_responded 08-25 → resolution slides to **09-18** (4-day suspension).
Plus: silence past the 3-day window → `impasse`/`nije_saglasan`; the 8-day answer clock never suspends; a second extension blocked; new-regime `log_answer` rejected without the warning fields; old-regime `log_answer` accepted without them; regime selection at the cutover boundary (filed 07-31 → old, 08-01 → new); register-number uniqueness/monotonicity; potvrda HTML contains the register number + no fiscal-receipt framing; retention eligibility flips at `filed_at + 2y`.

## Non-goals
- Customers table / consumer profiles; loyalty; B2B buyer capture.
- Read-access audit logging (SW-10).
- Services-vs-goods deadline divergence (memo §7 #5 — register records all; deadline logic is goods-framed; gate service-specific logic until confirmed).
- Weekend/holiday deadline rollover (unconfirmed ZUP applicability; safe = none).
- Auto-deletion at the retention floor (eligibility flag only).

## Acceptance criteria
- A complaint's regime is chosen at intake from its filing date and never changes; the deadline engine computes both regimes' clocks to the day, matching the memo's worked examples.
- The 8-day answer deadline never suspends; the resolution clock restarts (old) or suspends/resumes (new) on logged real-world events; silence → impasse; one extension max.
- New-regime answers are gated on the express warning; old-regime answers are not.
- Intake allocates a unique sequential register number and issues a printable potvrda; „bez odlaganja" has no timer.
- Retention is a 2-year floor (eligibility flag), PII is admin-gated, no consent UI.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
