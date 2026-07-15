# Sprint 1 Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the remaining Sprint 1 "app works at all" items — installer/config, logging, importer encoding, register ergonomics, out-of-stock override, go-live reset, and a full Serbian-diacritics UI sweep.

**Architecture:** Nine tasks executed in a fixed order so shared files (`lib.rs`, `Cargo.toml`, `RegisterScreen.tsx`, `SettingsScreen.tsx`, `types.ts`/adapters) never collide, with the diacritics sweep last as one atomic commit.

**Tech Stack:** Tauri v2, Rust, rusqlite/SQLite, React + TS, shadcn/ui, bun.

**Design doc:** `docs/superpowers/specs/2026-07-15-sprint1-completion-design.md`

## Global Constraints

- Branch: `feat/sprint1-completion` (already checked out).
- Money in integer minor units, quantities in milli-units; never floats.
- Operator strings are Serbian. `AppError`→`CommandError` via `From`; `require_admin` failure code is `"forbidden"`; `AppError::code()` is a `#[cfg(test)]` helper.
- All commits end with: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Verification gates (per task; full set before the final commit):
  - `bun run test` · `bun run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check` · `git diff --check`
- **Order is fixed:** 1→2→3→4→5→6→7→8→9. Task 9 (diacritics) is last and rewrites test assertions in lockstep.

---

### Task 1: Installer / window / single-instance / WAL (gap 0.7)

**Files:** `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/db/mod.rs`
**Not unit-testable** (bundle/window/plugin config on a macOS host) — verify by `cargo build` + config inspection. The db WAL change is covered by the existing db test suite staying green.

- [ ] **Step 1: tauri.conf.json** — set identity, window, and offline WebView2:
  - `"productName": "VantumPOS"` (was `"vantumpos"`)
  - `"identifier": "com.actaer.vantumpos"` (was `com.adnan.vantumpos`)
  - the single window object → `{ "label": "main", "title": "VantumPOS", "width": 1280, "height": 800, "minWidth": 1024, "minHeight": 720 }`
  - add to `"bundle"`: `"windows": { "webviewInstallMode": { "type": "offlineInstaller" } }`

- [ ] **Step 2: Add the single-instance plugin dependency** — in `src-tauri/Cargo.toml` `[dependencies]`, add `tauri-plugin-single-instance = "2"`.

- [ ] **Step 3: Register single-instance first** — in `src-tauri/src/lib.rs`, make it the FIRST `.plugin(...)` in the builder (before `tauri_plugin_opener`):

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
```

(`app.get_webview_window` needs `use tauri::Manager;` — already imported at lib.rs:12.)

- [ ] **Step 4: Enable WAL** — in `src-tauri/src/db/mod.rs` `open()`, add `PRAGMA journal_mode=WAL;` to the `execute_batch` PRAGMA block (before the `create_scalar_function` call):

```rust
        connection.execute_batch(
            r#"
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA journal_mode = WAL;
"#,
        )?;
```

- [ ] **Step 5: Verify + commit**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5` (expect success) and `cargo test --manifest-path src-tauri/Cargo.toml --lib db:: -- --test-threads=1` (expect PASS — migration count stays 6; WAL is a pragma). Confirm `tauri.conf.json` parses (build validates the schema).

```bash
git add src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/db/mod.rs
git commit -m "feat(app): real identity, offline WebView2 installer, single-instance, WAL

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: identifier change relocates `app_data_dir` (safe now, no installs). single-instance must reference window label `"main"` (capabilities pin `["main"]`). Cargo.lock will change (new plugin) — commit it.

---

### Task 2: Logging / panic hook / version display (gap 1.8)

**Files:** `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src/app/AppVersion.tsx` (new), `src/app/AppVersion.test.tsx` (new), `src/app/AppShell.tsx`, `src/App.test.tsx` (if a footer assertion trips)

- [ ] **Step 1: Failing component test** — create `src/app/AppVersion.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AppVersion } from "./AppVersion";
import type { SettingsService } from "@/services/ports";

function settingsWith(appVersion: string): SettingsService {
  return {
    getHealth: vi.fn(async () => ({
      backend: "local",
      appVersion,
      databasePath: "mock",
      migrated: true,
    })),
  } as unknown as SettingsService;
}

describe("AppVersion", () => {
  it("shows the app version from health", async () => {
    render(<AppVersion settings={settingsWith("0.1.0")} />);
    expect(await screen.findByText("Verzija 0.1.0")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run — expect FAIL** (`./AppVersion` missing): `bun run test src/app/AppVersion.test.tsx`

- [ ] **Step 3: Create the component** — `src/app/AppVersion.tsx`:

```tsx
import { useEffect, useState } from "react";

import type { SettingsService } from "@/services/ports";

export function AppVersion({ settings }: { settings: SettingsService }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    settings
      .getHealth()
      .then((health) => {
        if (!cancelled) setVersion(health.appVersion);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [settings]);

  if (!version) return null;
  return (
    <p className="px-2 text-xs text-sidebar-foreground/60">Verzija {version}</p>
  );
}
```

- [ ] **Step 4: Run — expect PASS**: `bun run test src/app/AppVersion.test.tsx`

- [ ] **Step 5: Render it in the sidebar footer** — in `src/app/AppShell.tsx`, import `AppVersion` and render `<AppVersion settings={services.settings} />` in the sidebar footer near the user/role block (the `SidebarFooter`/user area around AppShell.tsx:282-320). If `src/App.test.tsx` has a footer-copy assertion that now fails, update it to tolerate the added `Verzija` line.

- [ ] **Step 6: Add logging + panic hook (backend)** — in `src-tauri/Cargo.toml` add `tauri-plugin-log = "2"`. In `src-tauri/src/lib.rs`:
  - At the very top of `run()`, before the builder:
```rust
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));
```
  - Add `.plugin(tauri_plugin_log::Builder::new().target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None })).build())` to the builder (after single-instance/opener, before `.setup`).

- [ ] **Step 7: Full verify + commit**

Run: `bun run test && bun run build && cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings 2>&1 | tail -3`
Expected: all green.

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src/app/AppVersion.tsx src/app/AppVersion.test.tsx src/app/AppShell.tsx src/App.test.tsx
git commit -m "feat(app): file logging, panic hook, and app version in the sidebar

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: register the log plugin before `.setup`; the panic hook chains the default so stderr still fires. `log::error!` requires the `log` crate — `tauri-plugin-log` re-exports it or add `log = "0.4"` to deps if the macro isn't in scope.

---

### Task 3: Importer encoding — Windows-1250 fallback (gap 1.9)

**Files:** `src/lib/encoding.ts` (new), `src/lib/encoding.test.ts` (new), `src/app/import/ImportWizard.tsx`

- [ ] **Step 1: Failing test** — `src/lib/encoding.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { decodeCsv } from "./encoding";

// "Košulja" in Windows-1250: Š=0x8A? no — š=0x9A, but here use the exact cp1250 bytes.
// K o š u l j a  ->  0x4B 0x6F 0x9A 0x75 0x6C 0x6A 0x61
const cp1250Kosulja = new Uint8Array([0x4b, 0x6f, 0x9a, 0x75, 0x6c, 0x6a, 0x61]).buffer;

describe("decodeCsv", () => {
  it("auto-detects Windows-1250 when the bytes are not valid UTF-8", () => {
    const { text, encoding } = decodeCsv(cp1250Kosulja);
    expect(text).toBe("Košulja");
    expect(encoding).toBe("windows-1250");
  });

  it("keeps valid UTF-8 as UTF-8", () => {
    const utf8 = new TextEncoder().encode("Košulja").buffer;
    const { text, encoding } = decodeCsv(utf8);
    expect(text).toBe("Košulja");
    expect(encoding).toBe("utf-8");
  });

  it("honors an explicit encoding override", () => {
    const { text } = decodeCsv(cp1250Kosulja, "windows-1250");
    expect(text).toBe("Košulja");
  });
});
```

- [ ] **Step 2: Run — expect FAIL** (module missing): `bun run test src/lib/encoding.test.ts`

- [ ] **Step 3: Create `src/lib/encoding.ts`:**

```ts
export type CsvEncoding = "utf-8" | "windows-1250";

export function decodeCsv(
  bytes: ArrayBuffer,
  encoding?: CsvEncoding,
): { text: string; encoding: CsvEncoding } {
  if (encoding) {
    return { text: new TextDecoder(encoding).decode(bytes), encoding };
  }
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return { text, encoding: "utf-8" };
  } catch {
    return {
      text: new TextDecoder("windows-1250").decode(bytes),
      encoding: "windows-1250",
    };
  }
}
```

- [ ] **Step 4: Run — expect PASS**: `bun run test src/lib/encoding.test.ts`

- [ ] **Step 5: Wire into ImportWizard** — in `src/app/import/ImportWizard.tsx`, replace the `await file.text()` read (~line 205) with reading `await file.arrayBuffer()`, keep the bytes + chosen encoding in component state, and pass `decodeCsv(bytes, override).text` onward. Add a small `NativeSelect` on the preview/mapping card (options: `Automatski` = undefined override, `UTF-8`, `Windows-1250`) that re-decodes from the retained bytes on change. Verify with `bun run test` + `bun run build`.

- [ ] **Step 6: Commit**

```bash
git add src/lib/encoding.ts src/lib/encoding.test.ts src/app/import/ImportWizard.tsx
git commit -m "feat(import): decode Windows-1250 CSVs with auto-detect and a manual override

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: `TextDecoder("windows-1250")` is supported in the Tauri webview and vitest jsdom/Node — no polyfill. The Rust importer + `csvText: string` IPC contract are untouched, so importer.rs tests stay green.

---

### Task 4: Register ergonomics — percent discount + cash prefill (gap 1.10)

**Files:** `src/app/register/RegisterScreen.tsx`, `src/app/register/RegisterScreen.test.tsx`

- [ ] **Step 1: Failing tests** — add to `RegisterScreen.test.tsx`. First teach the in-file preview mock to honor percent (it currently only reads `type === "amount"` at ~lines 84 & 111): extend both the line-discount and receipt-discount handling so `{ type: "percent", basisPoints }` reduces the relevant subtotal by `basisPoints/10000`. Then add tests:

```ts
  it("applies a percent line discount typed as 20%", async () => {
    const user = userEvent.setup();
    await addProductToCart(user); // 1 x 159,99 = 15999
    await user.clear(screen.getByLabelText("Popust za Mleko 1 l"));
    await user.type(screen.getByLabelText("Popust za Mleko 1 l"), "20%");
    // 15999 - 20% = 12799.2 -> preview total reflects the percent, not a 20-para amount
    expect((await screen.findAllByText("127,99 RSD")).length).toBeGreaterThan(0);
  });

  it("prefills the cash field to the preview total", async () => {
    const user = userEvent.setup();
    await addProductToCart(user);
    expect(screen.getByLabelText("Gotovina primljeno")).toHaveValue("159.99");
  });
```

(Adjust the exact rounded strings to match `createSalePreview`'s rounding once the mock computes percent.)

- [ ] **Step 2: Run — expect FAIL**: `bun run test src/app/register/RegisterScreen.test.tsx`

- [ ] **Step 3: Implement** — in `src/app/register/RegisterScreen.tsx`:
  - Change the discount parser (currently `moneyDiscountFromInput`, always returns `{type:'amount'}`) so a trimmed value ending in `%` returns `{ type: "percent", basisPoints: Math.round(parseFloat(n) * 100) }` and otherwise `{ type: "amount", amountMinor }`. Apply to both line and receipt discount inputs. Add placeholder `20 ili 20%` to the discount inputs.
  - Prefill cash: add a `cashTouched` boolean state; a `useEffect` on the preview total sets `cashInput` to the formatted total **only when `!cashTouched`**; set `cashTouched` true in the cash `onChange`; reset `cashTouched=false` + clear on sale-complete. (The existing test at ~line 189 already types into cash — it must `clear()` first; update it.)

- [ ] **Step 4: Run — expect PASS**, then `bun run build`.

- [ ] **Step 5: Commit**

```bash
git add src/app/register/RegisterScreen.tsx src/app/register/RegisterScreen.test.tsx
git commit -m "feat(register): percent discounts (20%) and cash-tender prefill to total

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: the preview mock MUST compute percent or a green test masks a real bug. `Isprazni` (clear-cart) need not reset `cashTouched` (acceptable). Backend/contract unchanged.

---

### Task 5: Out-of-stock override — backend (gap 0.6)

**Files:** `src-tauri/src/commands/settings.rs`, `src-tauri/src/commands/sales.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Failing Rust tests** — add to the sales.rs test module:

```rust
    #[test]
    fn complete_sale_allows_oversell_with_per_sale_override() {
        // seed shift + a product with stock 1000 (allow_negative_stock = 0); request qty 2000
        // with allow_stock_override = Some(true) -> Ok, balance ends at -1000, one sale row.
    }

    #[test]
    fn complete_sale_allows_oversell_when_shop_setting_enabled() {
        // pre-insert settings row key='sales' value_json '{"allowOverselling":true}';
        // no per-sale flag -> Ok.
    }

    #[test]
    fn complete_sale_still_blocks_oversell_by_default() {
        // no shop setting, no override -> Err code "insufficient_stock" (guards the default).
    }
```

Write these using the existing sales.rs test seeding helpers (mirror `complete_sale_transaction_rolls_back_when_stock_is_insufficient` at ~939-971 for the seed shape); assert `error.code()` / final `inventory_balances.quantity_milli`.

- [ ] **Step 2: Run — expect FAIL** (`allow_stock_override` field missing).

- [ ] **Step 3: Add the `sales` settings key** — in `src-tauri/src/commands/settings.rs`, mirroring the receipt-settings shape:
  - `pub(crate) const SALES_SETTINGS_KEY: &str = "sales";`
  - `#[derive(...Serialize, Deserialize)] #[serde(rename_all="camelCase")] pub struct SalesSettings { #[serde(default)] pub allow_overselling: bool }` + `impl Default` (false) + `SalesSettingsRequest { pub allow_overselling: bool }`.
  - `#[tauri::command] settings_get_sales` → `load_sales_settings` (via `load_json_setting(state, SALES_SETTINGS_KEY, SalesSettings::default())`); `#[tauri::command] settings_update_sales` → `save_sales_settings` (calls `require_admin` then `save_json_setting`). Mirror `settings_get_receipt`/`settings_update_receipt` (settings.rs:139-150) and `load/save_receipt_settings`.

- [ ] **Step 4: Thread overselling through the sale** — in `src-tauri/src/commands/sales.rs`:
  - `CompleteSaleRequest`: add `#[serde(default)] pub allow_stock_override: Option<bool>`.
  - `validate_stock(lines, allow_overselling: bool)`: change the guard at line 486 to `if !allow_negative && !allow_overselling && current < required`.
  - In `complete_sale_transaction`: destructure `allow_stock_override` alongside the others; after `compute_sale`, read the shop setting **inside `tx`** (mirror `take_next_receipt_number`'s in-tx `SELECT value_json FROM settings WHERE key=?1` + parse, default false; use `SALES_SETTINGS_KEY`); `let overselling = shop_setting || allow_stock_override.unwrap_or(false);` and call `validate_stock(&computation.lines, overselling)`.
  - Extend the `use crate::commands::settings::...` import to bring in `SALES_SETTINGS_KEY` and `SalesSettings`.

- [ ] **Step 5: Register commands** — in `src-tauri/src/lib.rs`, add `commands::settings::settings_get_sales,` and `commands::settings::settings_update_sales,` after the receipt entries.

- [ ] **Step 6: Run — expect PASS**: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::sales:: commands::settings:: -- --test-threads=1`

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/commands/sales.rs src-tauri/src/lib.rs
git commit -m "feat(sales): shop-level and per-sale override for out-of-stock sales

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: default MUST stay blocking (guarded by `complete_sale_still_blocks_oversell_by_default` and the existing rollback test). Read the setting in-tx (not `load_json_setting`, which opens a nested connection). Preview path never calls `validate_stock` — keep it that way.

---

### Task 6: Out-of-stock override — frontend (gap 0.6)

**Files:** `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`, `src/app/settings/SettingsScreen.tsx`, `src/app/register/RegisterScreen.tsx`, plus their test files.

- [ ] **Step 1: Failing adapter + register tests**
  - `local-adapter.test.ts`: `getSalesSettings` → `settings_get_sales`; `updateSalesSettings` → `settings_update_sales` with `{ request }`.
  - `RegisterScreen.test.tsx`: update the existing `insufficient_stock` test (~209-228) — on the error, a confirm dialog „Nema dovoljno zaliha" appears; clicking „Ipak prodaj" re-invokes `completeSale` with `allowStockOverride: true` and completes.

- [ ] **Step 2: Run — expect FAIL.**

- [ ] **Step 3: Types + service wiring**
  - `types.ts`: add `SalesSettings { allowOverselling: boolean }`, `SalesSettingsRequest { allowOverselling: boolean }`, and `allowStockOverride?: boolean` on `CompleteSaleRequest`.
  - `ports.ts`: `getSalesSettings(): Promise<SalesSettings>` + `updateSalesSettings(request: SalesSettingsRequest): Promise<SalesSettings>` on `SettingsService`.
  - `local-adapter.ts`: `getSalesSettings: () => invoke<SalesSettings>("settings_get_sales")`, `updateSalesSettings: (request) => invoke<SalesSettings>("settings_update_sales", { request })`.
  - `mock-adapter.ts`: `let salesSettings = { allowOverselling: false }`; add `getSalesSettings`/`updateSalesSettings` in the settings block.

- [ ] **Step 4: SettingsScreen toggle** — add sales settings to the screen's load state + `reload()` Promise.all, and render a `Switch` (label „Dozvoli prodaju ispod stanja") wired to `updateSalesSettings` with a toast (mirror the receipt/company panels).

- [ ] **Step 5: Register override dialog** — in `RegisterScreen.tsx` completeSale catch: when `code === "insufficient_stock"`, store a pending-override sale and open a confirm `AlertDialog` (title „Nema dovoljno zaliha", actions „Odustani" / „Ipak prodaj"); on confirm, re-invoke `completeSale({ ...draft, payments, allowStockOverride: true })`.

- [ ] **Step 6: Run — expect PASS** (`bun run test`), then `bun run build`.

- [ ] **Step 7: Commit**

```bash
git add src/services/types.ts src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/app/settings/SettingsScreen.tsx src/app/register/RegisterScreen.tsx src/services/local-adapter.test.ts src/app/register/RegisterScreen.test.tsx
git commit -m "feat(register): confirm-and-oversell dialog + shop overselling toggle

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: `mock-adapter` must gain the two methods or `bun run build`/SettingsScreen.test.tsx break. `completeSale` in the mock never enforces stock — no override handling needed there.

---

### Task 7: Go-live reset — backend (gap 1.11)

**Files:** `src-tauri/src/commands/backup.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Failing Rust test** — in the backup.rs test module: seed catalog + users + a shift + a completed sale + inventory movement/balance + a bumped receipt counter; call `reset_trading_data` with the correct confirmation; assert `sales`/`sale_items`/`sale_payments`/`inventory_movements`/`shifts` are empty, `inventory_balances.quantity_milli` all 0, the `receipt_numbering` next_sequence_number reset to 1, and `products`/`users`/`tax_rates` rows intact. A wrong confirmation returns a validation error and changes nothing.

- [ ] **Step 2: Run — expect FAIL** (function missing).

- [ ] **Step 3: Implement `reset_trading_data`** — in `src-tauri/src/commands/backup.rs`, mirroring `restore_backup` (backup.rs:209-256):

```rust
const RESET_CONFIRMATION: &str = "OBRISI PODATKE";

pub fn reset_trading_data(state: &AppState, confirmation_text: &str) -> Result<(), AppError> {
    super::auth::require_admin(state)?;
    if confirmation_text.trim() != RESET_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrdite brisanje unosom teksta OBRISI PODATKE.",
            serde_json::json!({ "field": "confirmationText" }),
        ));
    }
    // safety backup first (reuse an allowed backup_type — no new migration)
    let _pre_reset = create_backup(
        state,
        CreateBackupRequest { backup_folder: None, backup_type: Some("automatic".to_string()) },
    )?;

    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM sales", [])?;              // cascades sale_items + sale_payments
    tx.execute("DELETE FROM inventory_movements", [])?; // no FK cascade — explicit
    tx.execute("DELETE FROM shifts", [])?;              // AFTER sales (sales.shift_id -> shifts)
    tx.execute("UPDATE inventory_balances SET quantity_milli = 0, updated_at = datetime('now')", [])?;
    // reset the receipt counter to 1, preserving the prefix
    tx.execute(
        "UPDATE settings
         SET value_json = json_set(value_json, '$.nextSequenceNumber', 1), updated_at = datetime('now')
         WHERE key = 'receipt_numbering'",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn backup_reset_trading_data(
    state: State<'_, AppState>,
    confirmation_text: String,
) -> Result<(), CommandError> {
    reset_trading_data(state.inner(), &confirmation_text).map_err(Into::into)
}
```

(If `json_set` on the receipt settings row is awkward because the row may be absent, instead read/parse/rewrite the `ReceiptSettings` like `save_receipt_settings` does, or simply `DELETE FROM settings WHERE key='receipt_numbering'` so it falls back to the default sequence 1. Pick whichever keeps the receipt-settings test green — deleting the key is simplest and resets to default.)

- [ ] **Step 4: Register** — in `src-tauri/src/lib.rs`, add `commands::backup::backup_reset_trading_data,`.

- [ ] **Step 5: Run — expect PASS + commit**

```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/lib.rs
git commit -m "feat(backup): admin-gated go-live reset of trading data with safety backup

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: FK order — delete `shifts` AFTER `sales`. `inventory_movements` has no FK cascade — explicit delete. Confirmation mirrors restore. Reused backup_type stays within the existing CHECK — no migration, so the migrate-count test stays 6.

---

### Task 8: Go-live reset — frontend (gap 1.11)

**Files:** `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`, `src/app/settings/SettingsScreen.tsx`, plus test files.

- [ ] **Step 1: Failing adapter test** — `resetTradingData("OBRISI PODATKE")` → `backup_reset_trading_data` with `{ confirmationText: "OBRISI PODATKE" }`.
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3: Wire service** — `ports.ts`: `resetTradingData(confirmationText: string): Promise<void>` on `BackupService`; `local-adapter.ts`: `resetTradingData: (confirmationText) => invoke<void>("backup_reset_trading_data", { confirmationText })`; `mock-adapter.ts`: a no-op `async resetTradingData() {}`.
- [ ] **Step 4: SettingsScreen control** — a destructive control in the backup/data panel with a typed-confirmation dialog (mirror restore's confirmation UX), calling `resetTradingData`; on success, re-fetch session/shift (the open shift is now gone) — surface a toast „Podaci obrisani".
- [ ] **Step 5: Run — expect PASS** (`bun run test` + `bun run build`), then commit.

```bash
git add src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/app/settings/SettingsScreen.tsx src/services/local-adapter.test.ts
git commit -m "feat(settings): go-live reset control with typed confirmation

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 9: Serbian diacritics + locale sweep (gap 1.12) — LAST, ATOMIC

**Files:** all user-facing string sites in `src/` and `src-tauri/` + every test that exact-matches a Serbian string + `mock-adapter.ts` messages. (Scout enumerated ~24 source files, ~90 frontend test assertions, 2 backend test assertions.)

This is one coherent, atomic change — source strings and their test assertions must move together or `bun run test` goes red.

- [ ] **Step 1: Establish the mapping** — enumerate ASCII Serbian words → diacritic forms actually used in the UI (e.g. `Podesavanja→Podešavanja`, `Kolicina→Količina`, `Zavrsi→Završi`, `Racun→Račun`, `Sacuvaj→Sačuvaj`, `Pretrazi→Pretraži`, `Nacin→Način`, `Storniraj/Storniranje`, `Ostecen→Oštećen`, `sifra→šifra`, `Pocetno→Početno`, error messages, etc.). Do **per-string-literal** edits — never a repo-wide `sed`.

- [ ] **Step 2: Sweep frontend source** — update Serbian literals across `src/app/**` (navigation.ts, RegisterScreen, SettingsScreen, ReportsScreen, InventoryScreen, CatalogModule, ImportWizard, AppShell, FirstRunTaxSetup, ReceiptsScreen) and `src/services/mock-adapter.ts` messages. Normalize locale helpers to `sr-Latn-RS` in `format.ts`, `money.ts`, `chart.tsx`.

- [ ] **Step 3: Sweep backend source** — update Serbian operator messages in `src-tauri/src/commands/*.rs` and `importer.rs`. **Do NOT touch** `text.rs`'s fold map/fixtures, the catalog folding test fixtures, or importer CSV header sample data.

- [ ] **Step 4: Update every test assertion in lockstep** — grep both suites for the old ASCII strings and update each `findByText`/`getByRole({name})`/`getByLabelText`/`assert_eq!(...message...)` to the diacritic form. Frontend tests assert against `mock-adapter` messages, so those must match the mock (not necessarily the backend) — keep mock and backend messages identical to avoid drift.

- [ ] **Step 5: Full verification**

```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git diff --check
```
Iterate until every string assertion matches. A useful loop: run `bun run test` / `cargo test`, read each failure's expected-vs-actual, fix the one drifted literal, repeat.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(i18n): correct Serbian diacritics (šđčćž) across the UI and messages

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Watch: over-eager replace can corrupt `text.rs` fold data, importer header fixtures, or English identifiers — per-literal only. Verify a search smoke-test that `Račun`/`Košulja`-type strings still fold and match after the sweep.

---

## Verification summary

After Task 9: real app identity + offline installer + single-instance + WAL; file logging + panic hook + visible version; Windows-1250 import; `20%` discounts and cash prefill; one-click oversell (or shop switch); admin go-live reset with safety backup; and a correctly-spelled Serbian UI. All six gates green. Deliberately deferred (design Non-goals): Cyrillic, admin gate on oversell, Rust-side CSV decoding, a dedicated `pre_reset` backup type, surgical reset, About dialog.
