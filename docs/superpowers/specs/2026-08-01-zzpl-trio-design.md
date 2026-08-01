# SW-10 + SW-13 + SW-17 — ZZPL Trio — Design

**Date:** 2026-08-01 · **Status:** proposed · **Verified law:** [REMAINING-SW-VERIFIED-RULES.md](../../REMAINING-SW-VERIFIED-RULES.md)

Three roadmap items that share one foundation: the audit log (SW-10), the employee lifecycle and purge (SW-13), and the internal breach record (SW-17) — plus the **čl. 47 evidencija radnji obrade generator** (requirement 28), which SW-17 has a hard dependency on and which is, on its own, the cheapest and most likely inspection finding for a three-employee shop.

Every requirement traces to a numbered item in §4 of the verified rule set. `[PRUDENTIAL]` items must never be presented to the operator as a legal duty.

---

## 0. What verification changed

**SW-10 is not a legal duty.** ZZPL čl. 50 st. 5 is an instruction-control duty, not a record-keeping one; logging is absent from čl. 50 st. 2's candidate measures; the two express logging duties (čl. 48, čl. 51 st. 2 t. 6–7) are both confined to a *nadležni organ … u posebne svrhe*; and **čl. 50 appears nowhere in čl. 95**, so no prekršaj attaches. We keep the feature and relabel it (req. 1). The honest hooks are **čl. 41 st. 1** (accountability — the controller must be *able to demonstrate*) and **čl. 46**, which *is* penalised and is the real hook for remote support.

**SW-13's stated basis was wrong.** ZEOR čl. 5's 25 fields contain no sale, void, till or KEP attribution. Ledger persistence rests on **ZoRač čl. 8 st. 4 + čl. 28** (req. 25).

**SW-17's citation survived**, but the field set must go well beyond the three čl. 52 st. 6 elements, because st. 7 demands whole-article compliance (req. 44).

---

## 1. Migration v18

### 1.1 `audit_events` (SW-10)

Append-only, hash-chained (req. 7). Columns: `id`, `at` (RFC3339), `actor_user_id` (FK, **never a free-text name**), `action` (closed enum mirroring čl. 48 st. 1 — `unos | menjanje | uvid | otkrivanje | uporedjivanje | brisanje`), `object_type`, `object_id` (opaque internal id), `reason_code` (**required for `uvid` and `otkrivanje`** — čl. 48 st. 2 requires *razlog*), `recipient` (disclosures only), `support_session_id`, `prev_hash`, `hash`.

**Hard exclusions enforced in code, not policy** (req. 4): no before/after value payloads, no JMBG, no card PAN, no address or phone, no free-text operator notes, and **no search query strings** — a query for a customer's name *is* personal data about that customer. A log that accumulates personal data breaches čl. 5 st. 1 t. 3 and enlarges the very attack surface it exists to shrink.

There is no owner-facing edit or delete path. An owner-editable audit log proves nothing, and proving something is the entire reason it exists.

### 1.2 `support_sessions` (SW-10, req. 2)

The **shop-side per-session approval**: `granted_by`, `granted_at`, `scope`, `expires_at`, `started_at`, `ended_at`, `revoked_at`. That approval record **is** the čl. 46 nalog, and it is worth more evidentially than the session log itself. This is the `[LEGAL]` half of SW-10; the log is the `[PRUDENTIAL]` half.

### 1.3 `employees` split (SW-13, req. 19)

**Three physically separate stores, three lifecycles:**

| Class | Contents | Lifecycle |
|---|---|---|
| **A — Personnel** | the 25 ZEOR čl. 5 fields | forever; **never purgeable by any UI action**; never on a till screen |
| **B — Ledger attribution** | a stable surrogate `employee_id` **only** | retained with the accounting document it sits in |
| **C — Credentials & access** | PIN/password hash + salt, login history | **the only class the purge job may touch** |

**Never denormalise `ime`, `prezime` or `matični broj` onto transaction rows** (req. 20). A name copied into a sale row is simultaneously an accounting record that cannot be deleted (ZoRač čl. 8 st. 4) and a personal-data record subject to minimisation — an unresolvable conflict of our own making. Surrogate id + lookup avoids it, and pseudonimizacija is expressly blessed by čl. 42 st. 1 t. 1.

Today's `users` table mixes all three classes. v18 adds a `personnel_records` table (class A) and leaves `users` as the account/credential store (class C), with the ledger already referencing `users.id` as the surrogate (class B) — so **no transaction row changes**.

### 1.4 `data_breaches` (SW-17, req. 44)

Beyond the three čl. 52 st. 6 fields: an **immutable `saznanje_at`** (the 72 h clock start — čl. 52 st. 1 says *"od saznanja"*, not from the incident), separate `occurred_at` and `discovered_at`, `risk_outcome`, an explicit `notify_decision` **with reasoning**, `poverenik_notified_at`, a `delay_reason` that becomes **mandatory once 72 h have elapsed since saznanje** (st. 2), and a separate čl. 53 block recording whether affected individuals were told and, if not, which čl. 53 st. 3 exception was relied on.

**Log first, decide notifiability second** (req. 43). Čl. 52 st. 6 covers *"svaku povredu"*, so notifiability is a **derived flag on a record that always persists** — never a wizard gate that discards non-notifiable incidents.

### 1.5 `processing_activities` (req. 28)

The čl. 47 register, generated rather than hand-maintained: purpose, categories, recipients, retention per category (st. 1 t. 6), čl. 50 measures. **The register itself is kept `trajno` (st. 7)** — and that is a deliberate contrast with the audit log and the breach log, neither of which has a prescribed period.

---

## 2. Retention — three different answers, one table

All three reuse the shared `retention_policies` table SW-14 created.

- **Audit log** (req. 6): configurable, **never "forever"**. No provision fixes a period. **Do not copy čl. 47 st. 7's *"čuvaju se trajno"* onto it** — that governs the register of processing activities, and applying it would put the product in permanent breach of storage limitation. Floor above the ZoP čl. 84 window (1 y relative, 2 y absolute).
- **Credentials** (req. 21): the PIN/password hash and salt are purged **at termination**, not after a window. A grace window measured in days is defensible; a 12-month default is not.
- **Login/action history** (req. 22): no statutory floor and no statutory ceiling. Defensible outer bound 3 years (ZoR čl. 196); 1-year default defensible on ZoP čl. 84 st. 1. Ship a default, expose the setting, **record the chosen value in the čl. 47 register**.
- **Breach log** (req. 49): configurable, never hardcoded. **On expiry, redact or pseudonymise the personal-data-bearing fields rather than deleting the row** — čl. 52 st. 7 argues for keeping the compliance skeleton.
- **Personnel and payroll categories** (req. 26): **no configurable period at all.**

**The purge is automatic and time-driven, not request-driven** (req. 23) — čl. 5 st. 1 t. 5 and čl. 42 st. 2 are proactive controller duties, and čl. 30 is the request-driven layer on top. Each purge writes an audit line recording *what class* was purged and when; **the audit line must not contain the purged values.**

---

## 3. Structural guarantees

**Deactivation must be structurally incapable of deleting the personnel record** (req. 24): no "Delete employee" affordance anywhere, no cascade delete from the account table to the personnel table, **no `ON DELETE CASCADE` pointing at it**. Deactivate-never-reuse, with non-reuse enforced by a uniqueness constraint that survives deactivation.

**Ledger immutability is a hard product requirement** (req. 25): append-only with reversal/storno, never update-in-place or delete. Cite **ZoRač čl. 8 st. 4**, never ZEOR.

**Purpose-lock in code, not policy** (req. 5): adopt the čl. 48 st. 3 enum verbatim as SW-10's stated purpose — *ocena zakonitosti obrade · interni nadzor · obezbeđivanje integriteta i bezbednosti podataka · pokretanje i vođenje krivičnog postupka.*

---

## 4. Outputs

- **Poverenik audit export** (req. 8) — human-readable, by date range and by actor, modelled on čl. 48 st. 4. **Must render offline, from the till.**
- **Pravilnik 40/2019 Obrazac** (req. 45) — exported **verbatim** in its five-section structure, ending with the mesto/datum + Ime i prezime + Potpis block and a "Prilog:" slot. **Print/PDF only. There is no submission API — do not build one.** Cite Pravilnik **čl. 3** for the in-app countdown (flat 72 h).
- **čl. 47 register** (req. 28) — generated from configured purposes, recipients, retention rows and čl. 50 measures.
- **Employee čl. 23 notice for the audit log** (req. 9) — the log processes employees' personal data, so the shop owes them information. A genuine gap that costs nothing to close and is more defensible than the log itself.

---

## 5. Copy rules

- **Never tell the client ZZPL requires an audit log** (req. 1). The penalty cell reads as three explicit rows: `čl. 42 → čl. 95 st. 1 t. 20 → preduzetnik st. 4 → 20.000–500.000` · `čl. 46 → čl. 95 st. 1 t. 23 → same tier` · **`čl. 50 → no prekršaj prescribed`** — and *"no prekršaj is prescribed"*, never *"no consequence"*.
- **Do not present POS operator attribution as legally mandatory** (req. 27). It is internal control on legitimni interes, objectionable under čl. 37 st. 1, and must appear in the shop's čl. 23 notice. Claiming a mandate that does not exist is itself a čl. 5 st. 1 t. 1 transparency problem.
- **SW-17 roadmap wording** (req. 50): *"provides the record required of the rukovalac by ZZPL čl. 52 st. 6–7"* — never *"satisfies"*. **Print no fine for a documentation-only failure**; state the notification tier (preduzetnik 20.000–500.000, čl. 95 st. 4) and note that a missing log destroys the defence to that charge.
- **The breach log is itself a processing operation** (req. 48): its own čl. 47 entry, admin-only RBAC, encryption at rest, and **exclusion from routine exports, reports and support bundles**.

---

## 6. What must NOT be built

1. No value payloads, PII fields, free-text notes or **search query strings** in the audit log.
2. No owner-facing edit or delete path on the audit log.
3. No `trajno` retention on the audit log or the breach log.
4. No "Delete employee" affordance; no cascade delete reaching the personnel record.
5. No denormalised employee name or JMBG on any transaction row.
6. No Poverenik submission API — none exists.
7. No wizard that discards a non-notifiable breach.
8. No claim that ZZPL mandates an audit log, or that POS operator attribution is legally required.

---

## 7. Testing

**Rust:** the hash chain detects a tampered row; `uvid`/`otkrivanje` without a `reason_code` is rejected; the exclusion list is enforced at the write boundary (a payload containing a JMBG-shaped string is refused); purge touches class C only and never A or B; the credential hash is gone at termination; the purge audit line contains no purged value; the 72 h `delay_reason` becomes mandatory exactly at the boundary; a non-notifiable breach still persists; retention classes resolve per category with personnel non-configurable; migration data-survival following the `270796c` pattern.

**Frontend:** the support-approval step captures scope and duration; the audit export renders offline; no "Delete employee" control exists anywhere; the breach form never gates on notifiability; the čl. 47 register renders from configuration.

---

## 8. Open items

| # | Item | Handling |
|---|---|---|
| R-9 | Consequence of an unpenalised čl. 50 breach | Register says *"no prekršaj is prescribed"*; the Poverenik's corrective powers (opomena, nalog) are the route. Never *"no consequence"*. |
| R-3 | Whose awareness anchors the 72 h clock when a processor is involved | Record **both** the processor-awareness and controller-notified timestamps; do not silently pick one. |
| — | Actaer as obrađivač | Build the čl. 52 st. 3 path separately (req. 47) — a vendor→merchant notification artefact feeding the merchant's `saznanje`. |
