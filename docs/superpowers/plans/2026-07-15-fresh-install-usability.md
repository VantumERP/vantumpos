# Fresh-Install Usability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a fresh install usable and safe on day one — seed VAT rates behind one first-run question, and make Serbian product search find diacritic/case variants while the register never rings up the wrong item.

**Architecture:** Part A adds an admin-gated, idempotent `settings_seed_tax_rates` command and a small blocking `FirstRunTaxSetup` modal at the app root. Part B adds a shared `fold_text()` registered as a SQL `fold()` scalar function on every connection, rewrites the catalog search to fold both sides, and replaces the register's blind `items[0]` with exact-match / single / disambiguation-picker / not-found resolution.

**Tech Stack:** Tauri v2, Rust, rusqlite/SQLite, React + TypeScript, shadcn/ui, bun.

**Design doc:** `docs/superpowers/specs/2026-07-15-fresh-install-usability-design.md`

## Global Constraints

- Branch: `feat/fresh-install-usability` (already checked out).
- Money in integer minor units, quantities in milli-units; never floats.
- Operator-facing strings are Serbian.
- No fiscalization/cloud/hardware dependencies.
- All commits end with: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Verification gates (run at the end of each task; full set at the end of Task 4):
  - `bun run test`
  - `bun run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - `git diff --check`
- Error-code facts: `require_admin` failure → code `"forbidden"`; `AppError` → `CommandError` via `From`, and `CommandError.code` is the public field asserted in tests.

---

### Task 1: Backend `settings_seed_tax_rates` command

**Files:**
- Modify: `src-tauri/src/commands/settings.rs` (add command + internal fn)
- Modify: `src-tauri/src/lib.rs` (register the command in `generate_handler!`)
- Test: `src-tauri/src/commands/settings.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub fn seed_tax_rates(state: &AppState, in_vat_system: bool) -> Result<Vec<TaxRate>, AppError>` and `#[tauri::command] settings_seed_tax_rates`. Idempotent: seeds only when `tax_rates` is empty; returns the current rate list either way. Consumed by the frontend (Task 2).

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/commands/settings.rs` (uses existing `with_state`, `sign_in_admin`, `sign_in_cashier`):

```rust
    #[test]
    fn seed_tax_rates_creates_vat_rates_when_in_system() {
        with_state("seed_tax_rates_vat", |state| {
            sign_in_admin(state);
            let rates = super::seed_tax_rates(state, true).expect("seed should succeed");
            assert_eq!(rates.len(), 2);
            let bps: Vec<i64> = rates.iter().map(|r| r.rate_basis_points).collect();
            assert!(bps.contains(&2000) && bps.contains(&1000), "got {bps:?}");
        });
    }

    #[test]
    fn seed_tax_rates_creates_single_zero_rate_when_not_in_system() {
        with_state("seed_tax_rates_novat", |state| {
            sign_in_admin(state);
            let rates = super::seed_tax_rates(state, false).expect("seed should succeed");
            assert_eq!(rates.len(), 1);
            assert_eq!(rates[0].rate_basis_points, 0);
        });
    }

    #[test]
    fn seed_tax_rates_is_idempotent() {
        with_state("seed_tax_rates_idempotent", |state| {
            sign_in_admin(state);
            super::seed_tax_rates(state, true).expect("first seed");
            let rates = super::seed_tax_rates(state, false).expect("second seed is a no-op");
            assert_eq!(rates.len(), 2, "existing rates must not be overwritten");
        });
    }

    #[test]
    fn seed_tax_rates_rejects_cashier() {
        with_state("seed_tax_rates_forbidden", |state| {
            sign_in_cashier(state);
            let error = super::seed_tax_rates(state, true).expect_err("cashier is forbidden");
            assert_eq!(error.code(), "forbidden");
        });
    }
```

`AppError::code()` is a `#[cfg(test)]` helper on `AppError` (`app_error.rs:67`), so no extra import is needed.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::settings::tests::seed_tax_rates -- --test-threads=1`
Expected: FAIL — `seed_tax_rates` does not exist (compile error).

- [ ] **Step 3: Add the internal function and command**

In `src-tauri/src/commands/settings.rs`, add the internal function next to `save_tax_rate`:

```rust
pub fn seed_tax_rates(state: &AppState, in_vat_system: bool) -> Result<Vec<TaxRate>, AppError> {
    super::auth::require_admin(state)?;
    {
        let mut conn = state.db().open()?;
        let existing: i64 =
            conn.query_row("SELECT COUNT(*) FROM tax_rates", [], |row| row.get(0))?;
        if existing == 0 {
            let tx = conn.transaction()?;
            if in_vat_system {
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('PDV 20%', 2000, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('PDV 10%', 1000, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('Bez PDV-a', 0, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
            }
            tx.commit()?;
        }
    }
    list_tax_rates(state)
}
```

Add the command next to `settings_save_tax_rate`:

```rust
#[tauri::command]
pub fn settings_seed_tax_rates(
    state: State<'_, AppState>,
    in_vat_system: bool,
) -> Result<Vec<TaxRate>, CommandError> {
    seed_tax_rates(state.inner(), in_vat_system).map_err(Into::into)
}
```

- [ ] **Step 4: Register the command**

In `src-tauri/src/lib.rs`, add to the `tauri::generate_handler![ … ]` list, right after `commands::settings::settings_save_tax_rate,`:

```rust
            commands::settings::settings_seed_tax_rates,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::settings:: -- --test-threads=1`
Expected: PASS (new tests plus existing settings tests).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/lib.rs
git commit -m "feat(settings): idempotent admin-gated seed_tax_rates command

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Frontend first-run VAT prompt

**Files:**
- Modify: `src/services/ports.ts` (`SettingsService`)
- Modify: `src/services/local-adapter.ts` (map `seedTaxRates`)
- Modify: `src/services/mock-adapter.ts` (implement `seedTaxRates`)
- Create: `src/app/FirstRunTaxSetup.tsx`
- Modify: `src/app/AppShell.tsx` (render the component in the authenticated branch)
- Test: `src/app/FirstRunTaxSetup.test.tsx`, `src/services/local-adapter.test.ts`

**Interfaces:**
- Consumes: `settings_seed_tax_rates` (Task 1).
- Produces: `SettingsService.seedTaxRates(inVatSystem: boolean): Promise<TaxRate[]>` and a `<FirstRunTaxSetup settings role />` component.

- [ ] **Step 1: Write the failing adapter test**

Add to `src/services/local-adapter.test.ts`, inside the `describe("local service adapter", …)` block:

```ts
  it("maps seedTaxRates to settings_seed_tax_rates", async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const services = createLocalServices(invoke);

    await services.settings.seedTaxRates(true);

    expect(invoke).toHaveBeenCalledWith("settings_seed_tax_rates", {
      inVatSystem: true,
    });
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun run test src/services/local-adapter.test.ts`
Expected: FAIL — `seedTaxRates` is not a function / not on the type.

- [ ] **Step 3: Add `seedTaxRates` to the port, local adapter, and mock**

In `src/services/ports.ts`, add to `interface SettingsService` (after `saveTaxRate`):

```ts
  seedTaxRates(inVatSystem: boolean): Promise<TaxRate[]>;
```

In `src/services/local-adapter.ts`, add to the settings object (after the `saveTaxRate` entry):

```ts
      seedTaxRates: (inVatSystem) =>
        invoke<TaxRate[]>("settings_seed_tax_rates", { inVatSystem }),
```

In `src/services/mock-adapter.ts`, add to the settings implementation (after `saveTaxRate`):

```ts
      async seedTaxRates(inVatSystem: boolean) {
        if (taxRates.length === 0) {
          taxRates = inVatSystem
            ? [
                { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
                { id: 2, name: "PDV 10%", rateBasisPoints: 1000, active: true },
              ]
            : [{ id: 1, name: "Bez PDV-a", rateBasisPoints: 0, active: true }];
        }
        return taxRates;
      },
```

- [ ] **Step 4: Run adapter test to verify it passes**

Run: `bun run test src/services/local-adapter.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing component test**

Create `src/app/FirstRunTaxSetup.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FirstRunTaxSetup } from "./FirstRunTaxSetup";
import type { SettingsService } from "@/services/ports";
import type { TaxRate } from "@/services/types";

function buildSettings(
  listResult: TaxRate[],
  seedTaxRates = vi.fn(async () => listResult),
): SettingsService {
  return {
    getHealth: vi.fn(),
    getCompanySettings: vi.fn(),
    updateCompanySettings: vi.fn(),
    listTaxRates: vi.fn(async () => listResult),
    saveTaxRate: vi.fn(),
    seedTaxRates,
    getReceiptSettings: vi.fn(),
    updateReceiptSettings: vi.fn(),
  } as unknown as SettingsService;
}

describe("FirstRunTaxSetup", () => {
  it("prompts an admin with no tax rates and seeds VAT on Da", async () => {
    const user = userEvent.setup();
    const seedTaxRates = vi.fn(async () => []);
    render(<FirstRunTaxSetup settings={buildSettings([], seedTaxRates)} role="admin" />);

    await user.click(await screen.findByRole("button", { name: "Da" }));
    expect(seedTaxRates).toHaveBeenCalledWith(true);
  });

  it("seeds the zero rate on Ne", async () => {
    const user = userEvent.setup();
    const seedTaxRates = vi.fn(async () => []);
    render(<FirstRunTaxSetup settings={buildSettings([], seedTaxRates)} role="admin" />);

    await user.click(await screen.findByRole("button", { name: "Ne" }));
    expect(seedTaxRates).toHaveBeenCalledWith(false);
  });

  it("renders nothing when rates already exist", async () => {
    render(
      <FirstRunTaxSetup
        settings={buildSettings([
          { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
        ])}
        role="admin"
      />,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByText("Da li ste u PDV sistemu?")).toBeNull();
  });

  it("renders nothing for a cashier", async () => {
    const settings = buildSettings([]);
    render(<FirstRunTaxSetup settings={settings} role="cashier" />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByText("Da li ste u PDV sistemu?")).toBeNull();
    expect(settings.listTaxRates).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 6: Run test to verify it fails**

Run: `bun run test src/app/FirstRunTaxSetup.test.tsx`
Expected: FAIL — module `./FirstRunTaxSetup` does not exist.

- [ ] **Step 7: Create the component**

Create `src/app/FirstRunTaxSetup.tsx`:

```tsx
import { useEffect, useState } from "react";

import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import type { SettingsService } from "@/services/ports";
import type { UserRole } from "@/services/types";

interface FirstRunTaxSetupProps {
  settings: SettingsService;
  role: UserRole;
}

export function FirstRunTaxSetup({ settings, role }: FirstRunTaxSetupProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    if (role !== "admin") {
      return;
    }

    let cancelled = false;
    settings
      .listTaxRates()
      .then((rates) => {
        if (!cancelled && rates.length === 0) {
          setOpen(true);
        }
      })
      .catch(() => {
        // Non-blocking: a load failure should not trap the operator in a modal.
      });

    return () => {
      cancelled = true;
    };
  }, [settings, role]);

  async function answer(inVatSystem: boolean) {
    setBusy(true);
    setError(undefined);
    try {
      await settings.seedTaxRates(inVatSystem);
      setOpen(false);
    } catch (caught) {
      setError((caught as { message?: string }).message ?? "Cuvanje nije uspelo.");
    } finally {
      setBusy(false);
    }
  }

  if (!open) {
    return null;
  }

  return (
    <AlertDialog open={open}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Da li ste u PDV sistemu?</AlertDialogTitle>
          <AlertDialogDescription>
            Podesite pocetne poreske stope. Ovo mozete kasnije promeniti u
            Podesavanjima.
          </AlertDialogDescription>
        </AlertDialogHeader>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <AlertDialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => answer(false)}
          >
            Ne
          </Button>
          <Button type="button" disabled={busy} onClick={() => answer(true)}>
            Da
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

- [ ] **Step 8: Run the component test to verify it passes**

Run: `bun run test src/app/FirstRunTaxSetup.test.tsx`
Expected: PASS

- [ ] **Step 9: Render it in AppShell's authenticated branch**

In `src/app/AppShell.tsx`, add the import near the other `@/app` imports:

```tsx
import { FirstRunTaxSetup } from "@/app/FirstRunTaxSetup";
```

Then, in the authenticated `return (…)` (the one starting `<TooltipProvider>` at the `const session = sessionState.session;` branch), insert the component as the first child inside `<TooltipProvider>`:

```tsx
    <TooltipProvider>
      <FirstRunTaxSetup settings={services.settings} role={session.user.role} />
      <SidebarProvider>
```

- [ ] **Step 10: Run frontend tests and commit**

Run: `bun run test src/app/FirstRunTaxSetup.test.tsx src/services/local-adapter.test.ts && bun run build`
Expected: PASS and a clean build.

```bash
git add src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/app/FirstRunTaxSetup.tsx src/app/FirstRunTaxSetup.test.tsx src/app/AppShell.tsx src/services/local-adapter.test.ts
git commit -m "feat(onboarding): first-run PDV question seeds tax rates

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Serbian search folding

**Files:**
- Modify: `src-tauri/Cargo.toml` (enable rusqlite `functions` feature)
- Create: `src-tauri/src/text.rs` (`fold_text`)
- Modify: `src-tauri/src/lib.rs` (declare `mod text;`)
- Modify: `src-tauri/src/db/mod.rs` (register the `fold` SQL function in `open`)
- Modify: `src-tauri/src/commands/catalog.rs` (fold the search pattern + query)
- Test: `src-tauri/src/text.rs` and `src-tauri/src/commands/catalog.rs`

**Interfaces:**
- Produces: `pub fn crate::text::fold_text(input: &str) -> String` and a deterministic SQL scalar `fold(text)` available on every connection. Consumed by the catalog search query.

- [ ] **Step 1: Create `text.rs` with `fold_text` + failing unit tests**

Create `src-tauri/src/text.rs`:

```rust
/// Folds Serbian Latin text for accent- and case-insensitive search:
/// lowercases (Unicode) then maps š→s, ž→z, č→c, ć→c, đ→dj.
pub fn fold_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.to_lowercase().chars() {
        match ch {
            'š' => out.push('s'),
            'ž' => out.push('z'),
            'č' | 'ć' => out.push('c'),
            'đ' => out.push_str("dj"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fold_text;

    #[test]
    fn folds_case_and_serbian_diacritics() {
        assert_eq!(fold_text("KOŠULJA"), "kosulja");
        assert_eq!(fold_text("Žabac"), "zabac");
        assert_eq!(fold_text("Đorđe"), "djordje");
        assert_eq!(fold_text("Čačak-Ćuprija"), "cacak-cuprija");
    }

    #[test]
    fn leaves_plain_ascii_and_digits_unchanged() {
        assert_eq!(fold_text("ABC 123"), "abc 123");
        assert_eq!(fold_text("8600000000010"), "8600000000010");
    }
}
```

- [ ] **Step 2: Declare the module and run the unit tests (expect fail before declaration)**

In `src-tauri/src/lib.rs`, add with the other `mod` declarations:

```rust
mod text;
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib text::tests -- --test-threads=1`
Expected: PASS (this is a pure function; it should go green immediately once the module is declared). If it fails, fix `fold_text` before proceeding.

- [ ] **Step 3: Enable the rusqlite `functions` feature**

In `src-tauri/Cargo.toml`, change the rusqlite dependency line to:

```toml
rusqlite = { version = "0.32", features = ["bundled", "backup", "functions"] }
```

- [ ] **Step 4: Write the failing search test**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/commands/catalog.rs` (uses existing `with_catalog_database`, which seeds tax rate id 1):

```rust
    #[test]
    fn search_products_folds_serbian_diacritics_and_case() {
        with_catalog_database("search_folds_serbian", |db| {
            db.open()
                .expect("database should open")
                .execute(
                    "INSERT INTO products (
                        name, sku, barcode, category_id, unit_of_measure, sale_price_minor,
                        purchase_price_minor, tax_rate_id, minimum_stock_milli, active,
                        created_at, updated_at)
                     VALUES ('KOŠULJA', 'KOS-1', '8600000000099', 1, 'kom', 250000, 180000, 1, 0, 1,
                        '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                    [],
                )
                .expect("kosulja should insert");

            for term in ["kosulja", "KOSULJA", "košulja", "KOŠULJA"] {
                let result = search_products(
                    db,
                    ProductSearchQuery {
                        search: Some(term.to_string()),
                        active: Some(true),
                        limit: Some(20),
                    },
                )
                .expect("search should succeed");
                assert!(
                    result.items.iter().any(|p| p.name == "KOŠULJA"),
                    "term {term} should find KOŠULJA"
                );
            }
        });
    }
```

- [ ] **Step 5: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog::tests::search_products_folds -- --test-threads=1`
Expected: FAIL — ASCII `LOWER()` cannot match the diacritic/upper variants yet.

- [ ] **Step 6: Register the `fold` SQL function in `Db::open`**

In `src-tauri/src/db/mod.rs`, add the import near the top:

```rust
use rusqlite::functions::FunctionFlags;
```

Then in `pub fn open`, after the `execute_batch(...)` PRAGMA block and before `Ok(connection)`:

```rust
        connection.create_scalar_function(
            "fold",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(crate::text::fold_text(&ctx.get::<String>(0)?)),
        )?;
```

- [ ] **Step 7: Fold both sides of the catalog search**

In `src-tauri/src/commands/catalog.rs`, in `list_products_for_connection`, change the pattern builder:

```rust
        .map(|value| format!("%{}%", crate::text::fold_text(value)));
```

and change the search clause in the SQL from:

```
             (?1 IS NULL
              OR LOWER(p.name) LIKE ?1
              OR LOWER(p.sku) LIKE ?1
              OR LOWER(COALESCE(p.barcode, '')) LIKE ?1)
```

to:

```
             (?1 IS NULL
              OR fold(p.name) LIKE ?1
              OR fold(p.sku) LIKE ?1
              OR fold(COALESCE(p.barcode, '')) LIKE ?1)
```

- [ ] **Step 8: Run the catalog suite to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog:: -- --test-threads=1`
Expected: PASS (new folding test plus the existing barcode/SKU/name search tests, which still resolve because folding is idempotent on ASCII/digits).

- [ ] **Step 9: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/text.rs src-tauri/src/lib.rs src-tauri/src/db/mod.rs src-tauri/src/commands/catalog.rs
git commit -m "feat(catalog): diacritic- and case-insensitive search via fold()

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Register scan / disambiguation

**Files:**
- Modify: `src/app/register/RegisterScreen.tsx` (`handleSearchSubmit`, matches state, picker JSX)
- Test: `src/app/register/RegisterScreen.test.tsx`

**Interfaces:**
- Consumes: the folded catalog search (Task 3) and `ProductSummary` (`id`, `name`, `sku`, `barcode`, `salePriceMinor`, `currentStockMilli`, `unitOfMeasure`).

- [ ] **Step 1: Write the failing tests**

Add to `src/app/register/RegisterScreen.test.tsx`, inside `describe("RegisterScreen", …)`. These build a services object whose `searchProducts` returns two products (mirroring how other tests override a service method):

```ts
  it("shows a picker for multiple matches and adds the chosen product", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    const productA = { ...product, id: 10, name: "Kosulja plava", sku: "KOS-P", barcode: "111" };
    const productB = { ...product, id: 11, name: "Kosulja bela", sku: "KOS-B", barcode: "222" };
    services.catalog.searchProducts = async () => ({ items: [productA, productB] });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "kosulja{enter}");

    await user.click(await screen.findByRole("button", { name: /Kosulja bela/ }));
    expect(await screen.findByText("Kosulja bela")).toBeInTheDocument();
  });

  it("adds the exact barcode match instead of the first result", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    const productA = { ...product, id: 10, name: "Kosulja plava", sku: "KOS-P", barcode: "1112223334445" };
    const productB = { ...product, id: 11, name: "Kosulja bela", sku: "KOS-B", barcode: "2223334445556" };
    services.catalog.searchProducts = async () => ({ items: [productA, productB] });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "2223334445556{enter}");

    expect(await screen.findByText("Kosulja bela")).toBeInTheDocument();
    expect(screen.queryByText("Kosulja plava")).toBeNull();
  });

  it("reports when nothing matches", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    services.catalog.searchProducts = async () => ({ items: [] });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "nepostojece{enter}");

    expect(await screen.findByText("Artikal nije pronadjen.")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run to verify they fail**

Run: `bun run test src/app/register/RegisterScreen.test.tsx`
Expected: FAIL — no picker; the exact-barcode test currently adds `items[0]` (Kosulja plava).

- [ ] **Step 3: Add matches state and resolution logic**

In `src/app/register/RegisterScreen.tsx`, add state near the other register state (after `const [search, setSearch] = useState("");`):

```tsx
  const [matches, setMatches] = useState<ProductSummary[]>([]);
```

Replace the body of `handleSearchSubmit` after the `setMessage(null);` line (the part that does `const result = …; const product = result.items[0]; if (!product) {…} addProduct(product); setSearch("");`) with:

```tsx
    const result = await services.catalog.searchProducts({
      search: query,
      active: true,
      limit: 20,
    });
    const items = result.items;

    if (items.length === 0) {
      setMatches([]);
      setMessage("Artikal nije pronadjen.");
      return;
    }

    const exact = items.find(
      (item) =>
        (item.barcode && item.barcode.toLowerCase() === query.toLowerCase()) ||
        item.sku.toLowerCase() === query.toLowerCase(),
    );

    if (exact) {
      addProduct(exact);
      setSearch("");
      setMatches([]);
      return;
    }

    if (items.length === 1) {
      addProduct(items[0]);
      setSearch("");
      setMatches([]);
      return;
    }

    setMatches(items);
```

- [ ] **Step 4: Add the picker JSX and a chooser helper**

In `src/app/register/RegisterScreen.tsx`, add a helper next to `addProduct`:

```tsx
  function chooseMatch(item: ProductSummary) {
    addProduct(item);
    setSearch("");
    setMatches([]);
  }
```

Then render the picker immediately after the search `</form>` (before the `{message && (…)}` block):

```tsx
      {matches.length > 0 && (
        <div
          role="listbox"
          aria-label="Rezultati pretrage"
          className="divide-y rounded-md border bg-card"
        >
          {matches.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => chooseMatch(item)}
              className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm hover:bg-accent"
            >
              <span className="min-w-0 truncate font-medium">{item.name}</span>
              <span className="shrink-0 tabular-nums text-muted-foreground">
                {formatRsd(item.salePriceMinor)} ·{" "}
                {formatQuantity(item.currentStockMilli)} {item.unitOfMeasure}
              </span>
            </button>
          ))}
        </div>
      )}
```

`formatRsd`, `formatQuantity`, and `ProductSummary` are already imported in this file.

- [ ] **Step 5: Run the register tests to verify they pass**

Run: `bun run test src/app/register/RegisterScreen.test.tsx`
Expected: PASS

- [ ] **Step 6: Full verification**

Run every gate:
```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git diff --check
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/app/register/RegisterScreen.tsx src/app/register/RegisterScreen.test.tsx
git commit -m "feat(register): resolve exact scans and disambiguate instead of ringing up items[0]

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Verification summary

After Task 4: a fresh DB prompts the admin once for PDV status (then products/imports work); Serbian product names are findable in any case/diacritic form; a scanned barcode resolves to exactly the right product; and an ambiguous text query shows a picker instead of silently ringing up the wrong item. Out of scope (noted in the design): Cyrillic search, sort-order collation, out-of-stock override (0.6), and installer (0.7).
