# Compliance P0 — Design

**Date:** 2026-07-16
**Status:** Approved design, ready for implementation plan
**Scope:** The five P0 software changes (SW-1…SW-5) plus the six founder-action legal templates from `docs/SERBIAN-LAW-COMPLIANCE.md` (§3 P0, §4). These close every vendor-side criminal/prekršaj exposure identified by the verified compliance research. P1 items (price history, reklamacije, printing, KEP, …) follow as separate staged cycles.

## Locked decisions

| Decision | Choice |
|---|---|
| Scope | **P0 now**, P1 staged afterwards. |
| Backup encryption key (SW-2) | **Admin passphrase** set in Settings; argon2-derived key; loud lost-passphrase warnings. |
| Legal templates | **All six drafted** as Serbian-language DRAFT documents in `docs/compliance/` for lawyer review. |

## A — SW-1: „OVO NIJE FISKALNI RAČUN" banner

Legal driver: PVFR čl. 2 st. 8–10 (adopted defensively); posture defense for ZF čl. 15 st. 1 tač. 5.

- **Post-sale dialog** (`RegisterScreen.tsx`, the `completedSale` Dialog): a prominent, non-dismissable banner **„OVO NIJE FISKALNI RAČUN"** rendered at the top AND bottom of the dialog body, visually ≥2× the line-item font size (e.g. `text-2xl font-bold` vs `text-sm` items), high-contrast. Replace the current `DialogDescription` „Lokalni račun" with „Interni pregled prodaje — nije fiskalni račun".
- **Receipt detail panel** (`ReceiptsScreen.tsx`): same banner above the item table (top only — the panel is an internal working screen, not a handout; the dialog is the one a customer could glimpse).
- **Design rule (documented in the code and posture memo):** no output anywhere may include a QR code, a „FISKALNI RAČUN" heading, or a PIB/PFR-number/brojač block. Any FUTURE print/PDF/export that itemizes a single sale must carry the banner top and bottom — recorded as a rule in the posture memo and as a code comment at the dialog.
- Tests: RTL assertions that the banner text renders in the post-sale dialog and receipt detail.

## B — SW-2: Encrypted backups (admin passphrase)

Legal driver: ZZPL čl. 50 st. 2 tač. 1 (kriptozaštita, first-named measure); čl. 53 st. 3 tač. 1 breach-notification safe-harbor for lost media.

**Threat model:** protect backup *media* (external drives, copied files, stolen USB). The live machine's DB is out of scope (OS-level concern).

**Key design (envelope):**
- Admin sets a **backup passphrase** in the Settings backup panel (min 8 chars). On set: generate a random 32-byte **data key**; derive a **wrapping key** from the passphrase via argon2id (random salt); store in the `settings` table under a new `backup_encryption` key: `{ salt, wrappedDataKey (AEAD-encrypted with wrapping key), dataKeyPlain }`. The plain data key stays local-only so scheduled/auto backups can encrypt without prompting; the wrapped copy travels inside each backup so a **fresh machine + passphrase** can always decrypt (disaster recovery).
- **File format** (new extension `.vpbk`): magic `VPBK1` + argon2 salt + wrapped data key + nonce + AEAD ciphertext (XChaCha20-Poly1305) of the SQLite snapshot. Single-shot encryption (shop DBs are MB-scale). Crate: `chacha20poly1305` (RustCrypto); argon2 already a dependency.
- **perform_backup:** online-backup to a temp file (existing logic), then if a passphrase is configured → encrypt to `<name>.vpbk`, delete the temp plaintext; if NOT configured → current plaintext `.sqlite3` behavior, and `BackupStatus` gains `encryptionConfigured: bool` so the UI shows a prominent warning „Rezervne kopije nisu šifrovane — postavite lozinku za šifrovanje".
- **restore_backup:** detect the `VPBK1` magic. Decrypt using the local plain data key if it matches the file's wrapped key; else require the new optional `passphrase` field on `RestoreBackupRequest` (fresh-machine path) — derive, unwrap, decrypt to a temp plain file, then the existing rusqlite restore. Wrong passphrase → clear validation error. Legacy plaintext backups keep restoring unchanged.
- **Commands:** `backup_set_passphrase(passphrase)` (admin; re-wraps the data key — old backups remain decryptable because the data key is unchanged) and the `encryptionConfigured` flag in `backup_get_status`.
- **UI:** passphrase set/change controls in the backup panel with the loud warning: „Ako izgubite lozinku, šifrovane rezervne kopije su NEPOVRATNO nečitljive na drugom računaru." Restore dialog gains an optional passphrase input shown when the picked file is `.vpbk`.
- Tests: encrypt→decrypt round-trip; wrong passphrase rejected; auto/manual backups produce `.vpbk` when configured; legacy plaintext restore still works; passphrase change keeps old backups decryptable.

## C — SW-3: Retention guards on reset/restore

Legal driver: ZoRač čl. 28 (5/10/20y tiers); ZPDV čl. 47 + ZPPPA čl. 114ž (**10-year** absolute horizon); ZPPPA čl. 175b (traceability); ZAG čl. 16/65 (d.o.o. destruction approval).

- **Migration v8** creates an append-only `compliance_log` table: `id, event_type TEXT CHECK(event_type IN ('trading_data_reset','backup_restored')), detail_json TEXT, user_id INTEGER REFERENCES users(id), created_at TEXT`. (Deliberately generic — becomes the seed of the P1 audit log SW-10.) `compliance_log` is **never** deleted by `reset_trading_data` and the reset event is written INSIDE the same transaction as the wipe.
- **reset_trading_data:** already forces a safety backup ✓. Add: write a `trading_data_reset` row (counts of deleted sales/movements/shifts + acting admin) in the wipe transaction. The reset UI dialog text now quotes the retention duty: „Zakon zahteva čuvanje evidencija do 10 godina (ZoRač čl. 28; ZPDV čl. 47). Pre brisanja se obavezno pravi rezervna kopija — čuvajte je trajno. Pravna lica ne smeju uništavati dokumentarni materijal bez pismenog odobrenja arhiva."
- **restore_backup:** after the restore + migrate, write a `backup_restored` row (source path + acting admin) into the restored DB.
- **Design rule (no code today):** when backup rotation/pruning is ever built, closed-year backups are excluded from pruning. Recorded in the posture memo.
- Tests: reset writes the tombstone and `compliance_log` survives the wipe; restore stamps the restored DB; the log rejects unknown event types.

## D — SW-4: Anti-evasion invariants (audit + tests + memo)

Legal driver: ZPPPA čl. 175a/175b (criminal: software serving turnover suppression — production, sale, or **possession**).

- **Invariant tests** (Rust, documenting the properties an inspector/expert would probe):
  1. Ledger append-only: assert behaviorally that `receipts_void`/`receipts_return_items` create counter-documents and never remove or monetarily alter existing rows — the `sales` row count only grows across void/return, and the original sale's monetary columns are byte-identical before/after.
  2. Voids/returns preserve the original document and its payments (rows before ⊆ rows after).
  3. `reset_trading_data` is impossible without admin + exact confirmation text, always produces a safety backup first, and always leaves a `compliance_log` tombstone (ties to SW-3).
- **Posture memo** `docs/compliance/memo-uskladjenost-fiskalizacije.md` (Serbian, DRAFT): VantumPOS's non-fiscal purpose, the append-only ledger design, counter-document model, logged reset — the standing rebuttal to any čl. 175b characterization, protecting both Actaer and the holding shop. References the invariant tests by name.

## E — SW-5: ESIR nudge + fiscal receipt number field

Legal driver: ZF čl. 4 st. 2 posture (ESIR-first workflow); ZZP26 čl. 63 st. 5 (proof-of-purchase linkage); ZEF čl. 3 (B2B corporate-card reconciliation).

- **Migration v8** (same migration as C): `ALTER TABLE sales ADD COLUMN esir_receipt_number TEXT;` (nullable).
- **Backend:** `receipts_set_esir_number(receiptId, esirReceiptNumber)` — any signed-in user (`require_session`), validates the receipt exists and is a `sale` document, trims/NULLs empty input, updates the column. The stored number is exposed on `ReceiptDetail` only; the post-sale dialog calls the command directly after `completeSale` returns (it has the sale id), so `CompletedSale` needs no new field.
- **Post-sale dialog:** a highlighted nudge line „Izdajte fiskalni račun na ESIR-u" + an optional input „Broj fiskalnog računa (ESIR)" with a save action calling the new command. Non-blocking — the dialog closes with or without it.
- **Receipt detail panel:** displays the stored ESIR number (and allows setting/correcting it, same command).
- Tests: Rust — command stores/clears the number, rejects a non-existent receipt and a `void`/`return` document; RTL — the nudge renders, entering a number invokes the service.

## F — Legal templates (`docs/compliance/`, Serbian, all headed „NACRT — pre upotrebe obavezna advokatska revizija")

1. `pitanje-purs.md` — the F-5 written question to PURS, verbatim from the report §6.1, with a context paragraph and the submission channel.
2. `ugovor-o-obradi-nacrt.md` — F-1 DPA skeleton: čl. 45 st. 3 identifiers, the eight st. 4 duties, st. 5 unlawful-instruction warning, sub-processor annex (st. 2/7), ≤24h breach clause (st. 3 čl. 52), audit clause, st. 8 note; flags the AnyDesk/transfer question for the lawyer.
3. `obavestenje-zaposlenima.md` — F-3 čl. 23 employee privacy notice (all items from the report).
4. `evidencija-obrade-cl47.md` — F-4 pre-filled čl. 47 records: controller (7 items, for the shop) + Actaer's processor record (4 items). Explicit „kontakt za zaštitu podataka — NE imenovati DPO" note.
5. `runbook-povreda-podataka.md` — F-11 one-page breach runbook (Actaer→shop ≤24h; shop→Poverenik ≤72h on the Pravilnik 40/2019 form to povredapodataka@poverenik.rs; internal log; high-risk direct notice; encrypted-backup exemption once SW-2 ships).
6. `checklist-onboarding-pilota.md` — F-8 pilot onboarding checklist (verify the shop's ESIR in the Registar: naziv/verzija/IB/no ukidanje; legal form + PDV status; „ESIR first" training incl. provokativna kupovina; korporacijska kartica/ZEF briefing; documented refusal of skip-ESIR requests; set the backup passphrase — ties to SW-2).

Plus the SW-4 memo (item D). Every template cites its legal basis inline and carries the DRAFT header.

## Non-goals (explicit)
- All P1/P2 items (price history, reklamacije, printing, KEP, audit log UI, cash trio, cenovnik export, employee lifecycle, work-time, onboarding flags, popis, …) — staged next.
- Encrypting the live database (out of threat model; OS concern).
- Backup rotation/pruning (doesn't exist; rule recorded for when it does).
- Any ESIR/PFR integration (gated on the PURS answer — F-5).

## Test plan summary
Rust: envelope crypto round-trips + wrong-passphrase + legacy restore; migration v8 (count→8, table+column present); tombstone written in-transaction and survives reset; restore stamps the restored DB; ESIR-number command happy/reject paths; anti-evasion invariant tests. Frontend: banner renders in both surfaces; encryption warning + passphrase controls; restore passphrase input for `.vpbk`; reset dialog retention text; ESIR nudge + save. All existing gates stay green.

## Acceptance criteria
- No sale-itemizing surface can be mistaken for a fiscal receipt; the banner is present and prominent.
- Backups are encrypted once a passphrase is set; a fresh machine + passphrase can restore; a lost passphrase is loudly warned about; unencrypted state is visibly flagged.
- Wiping/restoring trading data always leaves a permanent, in-transaction tombstone and quotes the retention duty; the safety backup remains forced.
- The anti-evasion invariants are executable tests + a citable memo.
- Every completed sale can carry the ESIR fiscal receipt number, nudged at sale completion.
- Six lawyer-ready DRAFT templates exist in `docs/compliance/`.
- All gates pass: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy … -D warnings`, `cargo fmt --check`, `git diff --check`.
