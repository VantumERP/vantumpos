# VantumPOS Module Specs

Date: 2026-06-18
Status: Draft handoff specs for separate implementation chats

## Purpose

The first merged foundation created the Tauri, SQLite, service-contract, test, and shadcn shell baseline. It did not implement real module screens. These specs split the remaining MVP into focused chunks so each module can be implemented in a separate chat without re-opening the whole product discussion.

Every implementation chat should read:

1. `docs/module-specs/00-shared-foundation.md`
2. The specific module spec for that chat
3. The approved product spec at `docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md`

## Recommended Order

1. `01-auth-shifts.md`
2. `02-catalog-products.md`
3. `03-inventory.md`
4. `04-register-sales.md`
5. `05-receipts-returns.md`
6. `06-reports.md`
7. `07-import.md`
8. `08-settings-backup.md`

Auth/shifts and catalog should happen before the register because sale completion depends on open shift, product, price, VAT, and stock state. Reports, import, and backup can be implemented after the transactional core exists.

## Chat Prompt Template

Use this shape for each new chat:

```text
Radis u D:\Projects\Actaer\vantumpos.

Procitaj:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/<MODULE_FILE>.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Implementiraj samo taj modul. Nemoj praviti genericke placeholder stranice. Svaki nav item koji diras mora da ima stvaran radni ekran, lokalni mock/loading/error state, servisni contract, Tauri command gde je potreban, Rust/SQLite testove za backend logiku i frontend testove za UI tok.

Koristi shadcn skill i proveri postojece shadcn komponente pre UI rada. Verifikuj sa:
- bun run test
- bun run build
- cd src-tauri; cargo test -- --test-threads=1
- cd src-tauri; cargo clippy --all-targets --all-features --locked -- -D warnings
- cd src-tauri; cargo fmt --check
- git diff --check
```

## Definition Of Done For Any Module

- The module has a distinct, useful screen. No `Spremno za modul`, no generic empty card as the final state.
- UI is backed by service interfaces, not hard-coded component-only state, except for explicit local fixtures in tests.
- Rust owns state-changing validation and transactions.
- Errors use structured backend codes and Serbian operator-facing messages.
- Frontend has loading, empty, validation, and error states.
- Tests cover at least one happy path and one important failure path.
- The app still runs as a local-first app without fiscalization, Medusa, or cloud dependencies.

