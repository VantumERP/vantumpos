# Reklamacija — zabrana naplate utvrđivanja nesaobraznosti (SW-7 residual) — Design

**Date:** 2026-08-02
**Status:** Approved
**Legal authority:** `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md` §4(c), §5, §6. That memo is the authority for the rule below — this spec encodes it, it does not restate law.
**Closes:** the open item recorded in `docs/SERBIAN-LAW-COMPLIANCE.md` row 23 and on the SW-7 roadmap row ("Residual 1").
**Builds on:** the SW-7 module shipped 31.07.2026 (`reklamacije.rs`, `commands/reklamacije.rs`, `reklamacije_docs.rs`, `ReklamacijeModule.tsx`) and the NEW-regime express-warning gate in `log_answer`.

## Scope

One rule: **35/2026 čl. 63 st. 3, second sentence** — *„Zabranjeno je da trgovac naplaćuje utvrđivanje nesaobraznosti."*

In scope: an operator-facing prohibition notice on NEW-regime complaints, a stored attestation gating the two transitions that close out a determination, and the sentence on both printed documents.

Out of scope: any till-side detection of a charge (see "Rejected" below), and read-access audit logging (row 22's residual, SW-10).

## The rule, and the three ways to get it wrong

1. **It is NEW-regime only.** 88/2021 čl. 55 st. 3 carries only the first sentence (*„Trgovac je dužan da primi izjavljenu reklamaciju"*) — **no fee ban**. Memo §6 lists presenting it as binding for OLD-regime complaints as a must-not-encode. The design makes this structural, not a UI convention (§3 below).
2. **It is not the same as the remedy being free.** Repair/replacement is free in *both* regimes (88/2021 čl. 51 st. 1 / 35/2026 čl. 56 st. 1). The copy must not conflate the two, or an OLD-regime shop will read the free remedy as a fee ban that does not bind it.
3. **It needs no new fine figure.** čl. 210 st. 1 tač. 24 penalises breach of čl. 63 st. **3** alongside st. 4 and 6–12, so `legal.rs::reklamacija_breach` already carries the correct tier-resolved amount. Nothing here introduces a figure, and nothing may.

## Confirmed engineering calls

| Call | Decision |
|---|---|
| What "block/warn" means | **Warn + attest.** There is no chargeable-service concept in the schema to block against — every `sale_items` row is a catalog product and no sale references a reklamacija. See "Rejected". |
| Attestation storage | A **column on `reklamacije`**, not a field in an event's `detail_json`. The flag is a property of the complaint, and "already attested" must be readable at the second gate without parsing JSON out of an event. |
| Gate points | **`log_answer` and `resolve_reklamacija`.** The answer is where the determination is recorded; resolve is the backstop, because `resolve_reklamacija` requires no prior answer (the memo §4(d) on-the-spot path) and would otherwise escape the gate. |
| Regime gating mechanism | `ReklamacijaView.no_fee_notice: Option<String>` — `Some` only for NEW. Renderers key off presence, so neither the frontend nor the potvrda renderer holds a regime conditional of its own. |
| Where the prohibition text lives | A **const in `reklamacije.rs`**, beside `MSG_NEW_ANSWER_WARNING`. It is statutory *text*, not a figure; `legal.rs` remains the home of figures only. |

## 1. Schema — migration v16

Two `ADD COLUMN`s, no table rebuild. No new tables or indexes, so `CORE_TABLES` / `EXPLICIT_INDEXES` and their count assertions are unchanged.

```sql
-- 35/2026 čl. 63 st. 3: the trader may not charge for determining nonconformity.
-- Attested once per complaint, NEW regime only; OLD-regime rows stay 0 forever
-- because the duty does not bind them (88/2021 čl. 55 st. 3 has no such ban).
ALTER TABLE reklamacije ADD COLUMN no_fee_attested INTEGER NOT NULL DEFAULT 0
    CHECK (no_fee_attested IN (0, 1));
ALTER TABLE reklamacije ADD COLUMN no_fee_attested_at TEXT;
```

Existing rows default to unattested, which is truthful — they were never asked. An in-flight NEW-regime complaint therefore needs the tick before it can be answered or closed. That is the gate working, not a migration defect.

`no_fee_attested_at` is nullable and set from the caller's `now` at the moment of attestation, never `datetime('now')` — consistent with the rest of the module.

## 2. Gate — `reklamacije.rs`

```rust
/// 35/2026 čl. 63 st. 3. NEW regime only: 88/2021 čl. 55 st. 3 has no fee ban,
/// and gating an old-regime record on it would assert a duty that does not bind.
/// Idempotent — once attested, later transitions only read the stored flag.
fn ensure_no_fee_attested(
    tx: &Connection,
    id: i64,
    regime: &str,
    attested_now: bool,
    now: &str,
) -> Result<(), AppError>
```

Behaviour:

- `regime != REGIME_NEW` → `Ok(())`. Nothing is read, nothing is written.
- stored flag already `1` → `Ok(())`. The operator ticks once, not once per transition.
- stored flag `0` and `attested_now == false` → `AppError::validation(MSG_NEW_NO_FEE, { "field": "noFeeAttested" })`.
- stored flag `0` and `attested_now == true` → set `no_fee_attested = 1`, `no_fee_attested_at = now`.

Call sites, both inside the existing transaction and before the event is appended, so a rejected transition persists nothing:

- `log_answer` — `AnswerInput` gains `no_fee_attested: bool`.
- `resolve_reklamacija` — gains a `no_fee_attested: bool` parameter.

Message const, beside `MSG_NEW_ANSWER_WARNING`:

```rust
const MSG_NEW_NO_FEE: &str = "Za novu reklamaciju potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).";
```

Not gated: `consumer_received_answer`, `consumer_responded`, `grant_extension`. They record the consumer's side or a consented date and involve no determination.

## 3. Copy and regime gating

```rust
/// Verbatim duty from 35/2026 čl. 63 st. 3, second sentence. NEW regime only.
const NO_FEE_NOTICE: &str = "Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). Otklanjanje nesaobraznosti — popravka ili zamena — je bez naknade po posebnoj odredbi (čl. 56 st. 1), i u starom i u novom režimu.";
```

`ReklamacijaView` gains three fields:

| Field | Type | Meaning |
|---|---|---|
| `no_fee_attested` | `bool` | stored flag |
| `no_fee_attested_at` | `Option<String>` | when, RFC3339 |
| `no_fee_notice` | `Option<String>` | `Some(NO_FEE_NOTICE)` iff `regime == REGIME_NEW` |

`no_fee_notice` is derived in `get_reklamacija` from the record's own frozen regime, exactly as `notice` already is. Every renderer keys off `Option` presence.

The second sentence of `NO_FEE_NOTICE` is load-bearing, and its first draft was wrong in a way worth recording. It read *„…a ne na sam način rešavanja reklamacije"* — which, by saying what the ban does *not* cover without saying what governs there instead, invites the reading that the repair itself may be charged for. It may not, in either regime (čl. 56 st. 1 / 51 st. 1). The sentence must therefore name the neighbouring rule rather than merely fence the ban off from it, so the two cannot be merged in either direction (mistake #2 above).

## 4. `legal.rs` correction

`reklamacija_breach`'s summary currently reads *„Nepostupanje po reklamaciji potrošača u propisanim rokovima je prekršaj."* Once the fee ban is surfaced, that is too narrow — čl. 210 st. 1 tač. 24 penalises breach of st. 3, which is not a deadline. Widen to a regime-neutral statement of the duty set:

> „Nepostupanje po propisanim obavezama u vezi sa reklamacijom potrošača je prekršaj."

It must stay regime-neutral: the same summary renders on OLD-regime records, so it may not name the NEW-only ban.

## 5. Surfaces

**`ReklamacijeModule.tsx`**

- Detail panel: render the prohibition whenever `noFeeNotice` is present — a standing duty, not gated on `overdue` (which governs the separate deadline advisory).
- `AnswerForm`, `ResolveForm`: a required checkbox, label *„Nije naplaćeno utvrđivanje nesaobraznosti (čl. 63 st. 3)"*, rendered only when `noFeeNotice` is present **and** `noFeeAttested` is false. Submitting unticked sets the existing `FieldError` — the client-side guard mirrors the backend gate, which stays authoritative.
- Once `noFeeAttested` is true the checkbox disappears from both forms; the standing prohibition line remains.

**`reklamacije_docs.rs`**

- `render_potvrda_html` — the sentence, gated on `view.no_fee_notice.is_some()`. The potvrda is per-record and regime-aware, so a pre-cutover complaint's potvrda must not carry it.
- `render_notice_html` — the sentence, unconditional. The prodajno-mesto notice is a static, forward-looking display document, so it states current law; it is not tied to any record's regime.

**Services layer**

- `types.ts`: `noFeeAttested`, `noFeeAttestedAt`, `noFeeNotice` on `ReklamacijaView`; `noFeeAttested` on `AnswerInput`.
- `ports.ts`: `resolve(id, nacin, eventDate, noFeeAttested)`.
- `local-adapter.ts`: pass `noFeeAttested` through to `reklamacija_resolve`.
- `mock-adapter.ts`: mirror the gate — reject an unattested NEW-regime answer/resolve, set the flag on success, and derive `noFeeNotice` from the frozen regime. A double that accepts what the backend rejects is worse than no double.

## 6. Tests (TDD — each observed failing first)

**Rust — `db/migrations.rs`**
- v16 adds both columns with the documented defaults, and a row seeded at v15 survives with `no_fee_attested = 0`.

**Rust — `reklamacije.rs`**
- NEW-regime `log_answer` is rejected when unattested and persists no event; accepted when attested, setting flag + timestamp.
- NEW-regime `resolve_reklamacija` is rejected when unattested — including the on-the-spot path where no answer was ever logged — and accepted when attested.
- Once attested at the answer, resolve succeeds without re-attesting, and `no_fee_attested_at` keeps its original value.
- OLD-regime `log_answer` and `resolve_reklamacija` succeed with `no_fee_attested: false`, the flag stays 0, and `no_fee_notice` is `None`.
- `no_fee_notice` is `Some` for a NEW-regime record.

**Rust — `legal.rs`**
- The widened summary is regime-neutral: it does not name the fee ban under either regime, and the existing tier/regime assertions still hold.

**Rust — `reklamacije_docs.rs`**
- The potvrda carries the sentence for a NEW-regime record and does **not** for an OLD-regime one.
- The prodajno-mesto notice always carries it.

**Frontend — `ReklamacijeModule.test.tsx`**
- The checkbox renders for a NEW-regime record and not for an OLD-regime one.
- Submitting the answer unticked shows the Serbian error and does not call the service.
- The checkbox is absent once `noFeeAttested` is true, while the prohibition line remains.

## Rejected

**Till-side detection.** Considered: flag catalog articles as diagnostic charges and warn when one is sold while a NEW-regime complaint is open. Rejected — paid diagnostics outside a complaint are lawful, so it fires false positives on lawful revenue; it depends on a per-article flag nobody will maintain; and no sale references a reklamacija, so "while a complaint is open" is a proxy, not the fact the statute turns on. A warning that is wrong about the law is worse than no warning.

**Reusing `LegalNotice` for the prohibition.** Rejected — `LegalNotice.penalty: None` already means *"the legal form is unanswered"* and drives a "set your pravna forma in Podešavanja → Profil" fallback in every renderer. A notice that legitimately has no figure of its own would render that fallback misleadingly. The penalty for this breach is `reklamacija_breach`, already on the view.

## Residual after this ships

Row 22's item stands: the register is admin-gated but read access is not logged (SW-10). This spec does not touch it.
