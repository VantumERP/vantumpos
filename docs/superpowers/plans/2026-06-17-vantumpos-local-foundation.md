# VantumPOS Local Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working local foundation for VantumPOS: test setup, SQLite migrations, Tauri health command, TypeScript service contracts, and a shadcn-based desktop app shell.

**Architecture:** This plan implements the first vertical slice from the approved local POS design. React uses TypeScript domain services, the local service adapter calls Tauri `invoke`, and Rust owns SQLite setup and migrations. This plan does not implement catalog, inventory, sale completion, import, backup, or reports workflows; those get separate plans after this foundation is working.

**Tech Stack:** Tauri 2, Rust 2021, rusqlite, SQLite, React 19, Vite 7, TypeScript, Tailwind v4, shadcn/ui base-mira, Vitest, Testing Library, Bun.

---

## Scope Split

The approved MVP spec covers several independent subsystems. Implementing all of it in one plan would create an oversized change with weak checkpoints. This plan produces a working, testable foundation that follow-up plans can build on.

Follow-up plans should cover:

- catalog and product management,
- inventory receiving/correction/write-off,
- register and sale completion,
- receipt search and void/refund,
- import wizard,
- backup/restore,
- reports.

## File Structure

Create or modify these files in this plan:

- Modify `package.json`: add frontend test scripts.
- Modify `vite.config.ts`: add Vitest configuration while keeping the Tauri dev server settings.
- Create `src/test/setup.ts`: Testing Library setup.
- Create `src/lib/money.ts`: money formatting/parsing helpers for RSD minor units.
- Create `src/lib/money.test.ts`: frontend unit tests for money helpers.
- Modify `src-tauri/Cargo.toml`: add SQLite and error dependencies.
- Create `src-tauri/src/app_error.rs`: internal backend errors and serializable command errors.
- Create `src-tauri/src/db/mod.rs`: database wrapper, connection setup, test helpers.
- Create `src-tauri/src/db/migrations.rs`: SQLite schema migrations.
- Create `src-tauri/src/state.rs`: shared Tauri app state and database path resolution.
- Create `src-tauri/src/commands/mod.rs`: command module exports.
- Create `src-tauri/src/commands/health.rs`: backend health command.
- Modify `src-tauri/src/lib.rs`: wire state and health command into Tauri.
- Create `src/services/types.ts`: frontend domain types for this foundation slice.
- Create `src/services/ports.ts`: frontend service interfaces.
- Create `src/services/local-adapter.ts`: local Tauri adapter with injectable `invoke`.
- Create `src/services/mock-adapter.ts`: deterministic mock service adapter for UI tests.
- Create `src/services/local-adapter.test.ts`: service adapter tests.
- Create `src/app/navigation.ts`: sidebar navigation model.
- Create `src/app/AppShell.tsx`: shadcn sidebar layout.
- Create `src/app/BackendStatus.tsx`: health status panel.
- Create `src/App.test.tsx`: app shell tests.
- Modify `src/App.tsx`: replace starter demo with the app shell.
- Modify `src/App.css`: remove starter CSS and keep Tailwind/shadcn theme setup.

## Task 1: Frontend Test Harness And Money Helpers

**Files:**
- Modify: `package.json`
- Modify: `vite.config.ts`
- Create: `src/test/setup.ts`
- Create: `src/lib/money.test.ts`
- Create: `src/lib/money.ts`

- [ ] **Step 1: Install frontend test dependencies**

Run:

```powershell
bun add -d vitest jsdom @testing-library/react @testing-library/jest-dom @testing-library/user-event
```

Expected: `package.json` and `bun.lock` update with the test dependencies.

- [ ] **Step 2: Add test scripts to `package.json`**

Replace the `scripts` object with:

```json
{
  "dev": "vite",
  "build": "tsc && vite build",
  "preview": "vite preview",
  "test": "vitest run",
  "test:watch": "vitest",
  "tauri": "tauri"
}
```

- [ ] **Step 3: Add Vitest config to `vite.config.ts`**

Replace `vite.config.ts` with:

```ts
/// <reference types="vitest/config" />

import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": new URL("./src", import.meta.url).pathname,
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: true,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
```

- [ ] **Step 4: Create `src/test/setup.ts`**

```ts
import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});
```

- [ ] **Step 5: Write the failing money helper tests**

Create `src/lib/money.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { formatRsd, parseRsdInput } from "./money";

describe("RSD money helpers", () => {
  it("formats integer minor units without floating point math", () => {
    expect(formatRsd(0)).toBe("0,00 RSD");
    expect(formatRsd(1234)).toBe("12,34 RSD");
    expect(formatRsd(123456)).toBe("1.234,56 RSD");
    expect(formatRsd(-1234)).toBe("-12,34 RSD");
  });

  it("parses Serbian decimal input into minor units", () => {
    expect(parseRsdInput("0")).toBe(0);
    expect(parseRsdInput("12,34")).toBe(1234);
    expect(parseRsdInput("1.234,56")).toBe(123456);
    expect(parseRsdInput("1234.56")).toBe(123456);
  });

  it("rejects invalid money input", () => {
    expect(() => parseRsdInput("")).toThrow("Unesite iznos.");
    expect(() => parseRsdInput("abc")).toThrow("Iznos nije ispravan.");
    expect(() => parseRsdInput("12,345")).toThrow("Iznos nije ispravan.");
  });
});
```

- [ ] **Step 6: Run the money tests to verify they fail**

Run:

```powershell
bun run test -- src/lib/money.test.ts
```

Expected: FAIL because `src/lib/money.ts` does not exist.

- [ ] **Step 7: Implement `src/lib/money.ts`**

```ts
const RSD_GROUP_SEPARATOR = ".";
const RSD_DECIMAL_SEPARATOR = ",";

export function formatRsd(minorUnits: number): string {
  assertInteger(minorUnits, "minorUnits");

  const sign = minorUnits < 0 ? "-" : "";
  const absolute = Math.abs(minorUnits);
  const dinars = Math.floor(absolute / 100);
  const paras = absolute % 100;

  return `${sign}${formatDinars(dinars)}${RSD_DECIMAL_SEPARATOR}${paras
    .toString()
    .padStart(2, "0")} RSD`;
}

export function parseRsdInput(input: string): number {
  const trimmed = input.trim();

  if (!trimmed) {
    throw new Error("Unesite iznos.");
  }

  const normalized = normalizeMoneyInput(trimmed);

  if (!/^-?\d+(\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Iznos nije ispravan.");
  }

  const [dinarsPart, parasPart = ""] = normalized.split(".");
  const sign = dinarsPart.startsWith("-") ? -1 : 1;
  const dinars = Math.abs(Number.parseInt(dinarsPart, 10));
  const paras = Number.parseInt(parasPart.padEnd(2, "0"), 10) || 0;

  return sign * (dinars * 100 + paras);
}

function normalizeMoneyInput(input: string): string {
  const compact = input.replace(/\s/g, "");
  const hasComma = compact.includes(",");
  const hasDot = compact.includes(".");

  if (hasComma) {
    return compact.replace(/\./g, "").replace(",", ".");
  }

  if (hasDot) {
    const parts = compact.split(".");
    const lastPart = parts[parts.length - 1];

    if (lastPart.length <= 2 && parts.length > 1) {
      return `${parts.slice(0, -1).join("")}.${lastPart}`;
    }

    return compact.replace(/\./g, "");
  }

  return compact;
}

function formatDinars(value: number): string {
  return value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, RSD_GROUP_SEPARATOR);
}

function assertInteger(value: number, name: string): void {
  if (!Number.isInteger(value)) {
    throw new Error(`${name} mora biti ceo broj.`);
  }
}
```

- [ ] **Step 8: Run frontend tests**

Run:

```powershell
bun run test -- src/lib/money.test.ts
```

Expected: PASS for all tests in `src/lib/money.test.ts`.

- [ ] **Step 9: Commit Task 1**

```powershell
git add package.json bun.lock vite.config.ts src/test/setup.ts src/lib/money.ts src/lib/money.test.ts
git commit -m "test: add frontend test harness and money helpers"
```

## Task 2: SQLite Schema And Migration Foundation

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/app_error.rs`
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Add Rust dependencies**

In `src-tauri/Cargo.toml`, add these dependencies under `[dependencies]`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
thiserror = "2"
time = { version = "0.3", features = ["formatting", "parsing", "serde"] }
```

- [ ] **Step 2: Write failing database migration tests**

Create `src-tauri/src/db/mod.rs` with only this test scaffold:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_database() {
        let db_path = test_database_path("migrates_empty_database");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let conn = db.open().expect("database should open");

        let user_tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('settings', 'users', 'products', 'sales')",
                [],
                |row| row.get(0),
            )
            .expect("table count should be readable");

        assert_eq!(user_tables, 4);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn migrations_are_idempotent() {
        let db_path = test_database_path("migrations_are_idempotent");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        db.migrate().expect("second migration run should succeed");

        let conn = db.open().expect("database should open");
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("migration count should be readable");

        assert_eq!(applied, 1);

        let _ = std::fs::remove_file(db_path);
    }
}
```

- [ ] **Step 3: Run Rust tests to verify they fail**

Run:

```powershell
cd src-tauri
cargo test db::tests -- --test-threads=1
```

Expected: FAIL because `Db` and `test_database_path` are not defined.

- [ ] **Step 4: Create `src-tauri/src/app_error.rs`**

```rust
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    InvalidState(String),
}

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Database(source) => {
                Self::new("database_error", format!("Greska baze podataka: {source}"))
            }
            AppError::Io(source) => {
                Self::new("file_system_error", format!("Greska fajl sistema: {source}"))
            }
            AppError::InvalidState(message) => Self::new("invalid_state", message),
        }
    }
}
```

- [ ] **Step 5: Create `src-tauri/src/db/migrations.rs`**

```rust
use rusqlite::{params, Connection};

use crate::app_error::AppError;

pub struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_local_pos_schema",
    sql: r#"
CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'cashier')),
    pin_hash TEXT,
    password_hash TEXT,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE shifts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    opened_at TEXT NOT NULL,
    closed_at TEXT,
    opening_cash_minor INTEGER NOT NULL DEFAULT 0,
    expected_cash_minor INTEGER NOT NULL DEFAULT 0,
    counted_cash_minor INTEGER,
    status TEXT NOT NULL CHECK (status IN ('open', 'closed')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE tax_rates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    rate_basis_points INTEGER NOT NULL CHECK (rate_basis_points >= 0),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    sku TEXT NOT NULL UNIQUE,
    barcode TEXT UNIQUE,
    category_id INTEGER REFERENCES categories(id),
    unit_of_measure TEXT NOT NULL DEFAULT 'kom',
    sale_price_minor INTEGER NOT NULL CHECK (sale_price_minor >= 0),
    purchase_price_minor INTEGER NOT NULL DEFAULT 0 CHECK (purchase_price_minor >= 0),
    tax_rate_id INTEGER NOT NULL REFERENCES tax_rates(id),
    minimum_stock_milli INTEGER NOT NULL DEFAULT 0,
    allow_negative_stock INTEGER NOT NULL DEFAULT 0 CHECK (allow_negative_stock IN (0, 1)),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE inventory_balances (
    product_id INTEGER PRIMARY KEY REFERENCES products(id) ON DELETE CASCADE,
    quantity_milli INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE inventory_movements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id),
    movement_type TEXT NOT NULL CHECK (movement_type IN ('receive', 'correction', 'write_off', 'sale', 'return', 'void')),
    quantity_milli INTEGER NOT NULL,
    reason TEXT,
    reference_type TEXT,
    reference_id INTEGER,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);

CREATE TABLE sales (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_receipt_number TEXT NOT NULL UNIQUE,
    shift_id INTEGER NOT NULL REFERENCES shifts(id),
    cashier_id INTEGER NOT NULL REFERENCES users(id),
    status TEXT NOT NULL CHECK (status IN ('completed', 'voided', 'refunded')),
    fiscal_status TEXT NOT NULL DEFAULT 'not_fiscalized' CHECK (fiscal_status IN ('not_fiscalized', 'fiscalized', 'failed')),
    original_sale_id INTEGER REFERENCES sales(id),
    subtotal_minor INTEGER NOT NULL,
    discount_minor INTEGER NOT NULL DEFAULT 0,
    tax_minor INTEGER NOT NULL,
    total_minor INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE sale_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    product_id INTEGER REFERENCES products(id),
    product_name TEXT NOT NULL,
    product_sku TEXT NOT NULL,
    product_barcode TEXT,
    quantity_milli INTEGER NOT NULL,
    unit_price_minor INTEGER NOT NULL,
    discount_minor INTEGER NOT NULL DEFAULT 0,
    tax_rate_basis_points INTEGER NOT NULL,
    tax_minor INTEGER NOT NULL,
    total_minor INTEGER NOT NULL
);

CREATE TABLE sale_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'card')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),
    created_at TEXT NOT NULL
);

CREATE TABLE import_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    import_type TEXT NOT NULL CHECK (import_type IN ('products', 'categories', 'initial_stock')),
    file_name TEXT NOT NULL,
    column_mapping_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'validated', 'completed', 'failed')),
    total_rows INTEGER NOT NULL DEFAULT 0,
    error_rows INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE import_job_rows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    import_job_id INTEGER NOT NULL REFERENCES import_jobs(id) ON DELETE CASCADE,
    row_number INTEGER NOT NULL,
    raw_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('valid', 'warning', 'error', 'imported', 'skipped')),
    message TEXT
);

CREATE TABLE backup_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_type TEXT NOT NULL CHECK (backup_type IN ('manual', 'automatic')),
    path TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    error_message TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_products_name ON products(name);
CREATE INDEX idx_products_barcode ON products(barcode);
CREATE INDEX idx_inventory_movements_product ON inventory_movements(product_id, created_at);
CREATE INDEX idx_sales_created_at ON sales(created_at);
CREATE INDEX idx_sales_shift ON sales(shift_id);
CREATE INDEX idx_sale_items_product ON sale_items(product_id);
"#,
}];

pub fn run_migrations(conn: &mut Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS _migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
"#,
    )?;

    let tx = conn.transaction()?;

    for migration in MIGRATIONS {
        let already_applied: i64 = tx.query_row(
            "SELECT COUNT(*) FROM _migrations WHERE version = ?1",
            params![migration.version],
            |row| row.get(0),
        )?;

        if already_applied == 0 {
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                params![migration.version, migration.name],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 6: Replace `src-tauri/src/db/mod.rs` with implementation plus tests**

```rust
mod migrations;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;

use crate::app_error::AppError;

#[derive(Clone, Debug)]
pub struct Db {
    path: Arc<PathBuf>,
}

impl Db {
    pub fn new(path: PathBuf) -> Result<Self, AppError> {
        ensure_parent_directory(&path)?;
        let db = Self {
            path: Arc::new(path),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn open(&self) -> Result<Connection, AppError> {
        let conn = Connection::open(self.path.as_path())?;
        conn.execute_batch(
            r#"
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
"#,
        )?;
        Ok(conn)
    }

    pub fn migrate(&self) -> Result<(), AppError> {
        let mut conn = self.open()?;
        migrations::run_migrations(&mut conn)
    }
}

fn ensure_parent_directory(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(())
}

#[cfg(test)]
pub fn test_database_path(test_name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("vantumpos-{test_name}-{unique}.sqlite3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_database() {
        let db_path = test_database_path("migrates_empty_database");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let conn = db.open().expect("database should open");

        let user_tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('settings', 'users', 'products', 'sales')",
                [],
                |row| row.get(0),
            )
            .expect("table count should be readable");

        assert_eq!(user_tables, 4);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn migrations_are_idempotent() {
        let db_path = test_database_path("migrations_are_idempotent");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        db.migrate().expect("second migration run should succeed");

        let conn = db.open().expect("database should open");
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("migration count should be readable");

        assert_eq!(applied, 1);

        let _ = std::fs::remove_file(db_path);
    }
}
```

- [ ] **Step 7: Expose modules temporarily in `src-tauri/src/lib.rs` for tests**

At the top of `src-tauri/src/lib.rs`, before the existing command, add:

```rust
mod app_error;
mod db;
```

- [ ] **Step 8: Run Rust migration tests**

Run:

```powershell
cd src-tauri
cargo test db::tests -- --test-threads=1
```

Expected: PASS for `migrates_empty_database` and `migrations_are_idempotent`.

- [ ] **Step 9: Commit Task 2**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/app_error.rs src-tauri/src/db
git add src-tauri/src/lib.rs
git commit -m "feat: add local sqlite migration foundation"
```

## Task 3: Tauri App State And Health Command

**Files:**
- Create: `src-tauri/src/state.rs`
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/health.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing health command tests**

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod health;
```

Create `src-tauri/src/commands/health.rs` with only this test scaffold:

```rust
#[cfg(test)]
mod tests {
    use crate::commands::health::build_health_response;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    #[test]
    fn health_response_reports_database_path_and_migration_state() {
        let db_path = test_database_path("health_response_reports_database_path_and_migration_state");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let state = AppState::new(db);

        let response = build_health_response(&state, "0.1.0").expect("health should build");

        assert_eq!(response.backend, "local");
        assert_eq!(response.app_version, "0.1.0");
        assert!(response.database_path.ends_with(".sqlite3"));
        assert!(response.migrated);

        let _ = std::fs::remove_file(db_path);
    }
}
```

- [ ] **Step 2: Run health tests to verify they fail**

Run:

```powershell
cd src-tauri
cargo test commands::health::tests -- --test-threads=1
```

Expected: FAIL because `state`, `AppState`, and `build_health_response` are missing.

- [ ] **Step 3: Create `src-tauri/src/state.rs`**

```rust
use std::path::PathBuf;

use tauri::Manager;

use crate::app_error::AppError;
use crate::db::Db;

#[derive(Clone, Debug)]
pub struct AppState {
    db: Db,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }
}

pub fn resolve_database_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::InvalidState(format!("Ne mogu da pronadjem folder aplikacije: {error}"))
    })?;

    std::fs::create_dir_all(&app_data_dir)?;
    Ok(app_data_dir.join("vantumpos.sqlite3"))
}
```

- [ ] **Step 4: Replace `src-tauri/src/commands/health.rs` with implementation plus tests**

```rust
use serde::Serialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHealth {
    pub backend: &'static str,
    pub app_version: String,
    pub database_path: String,
    pub migrated: bool,
}

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>) -> Result<AppHealth, CommandError> {
    build_health_response(state.inner(), env!("CARGO_PKG_VERSION")).map_err(Into::into)
}

pub fn build_health_response(
    state: &AppState,
    app_version: impl Into<String>,
) -> Result<AppHealth, AppError> {
    let conn = state.db().open()?;
    let migration_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))?;

    Ok(AppHealth {
        backend: "local",
        app_version: app_version.into(),
        database_path: state.db().path().display().to_string(),
        migrated: migration_count > 0,
    })
}

#[cfg(test)]
mod tests {
    use crate::commands::health::build_health_response;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    #[test]
    fn health_response_reports_database_path_and_migration_state() {
        let db_path = test_database_path("health_response_reports_database_path_and_migration_state");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let state = AppState::new(db);

        let response = build_health_response(&state, "0.1.0").expect("health should build");

        assert_eq!(response.backend, "local");
        assert_eq!(response.app_version, "0.1.0");
        assert!(response.database_path.ends_with(".sqlite3"));
        assert!(response.migrated);

        let _ = std::fs::remove_file(db_path);
    }
}
```

- [ ] **Step 5: Replace `src-tauri/src/lib.rs`**

```rust
mod app_error;
mod commands;
mod db;
mod state;

use db::Db;
use state::{resolve_database_path, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_path = resolve_database_path(&app.handle())?;
            let db = Db::new(db_path)?;
            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::get_app_health
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Run Rust tests**

Run:

```powershell
cd src-tauri
cargo test -- --test-threads=1
```

Expected: PASS for all Rust tests.

- [ ] **Step 7: Run Tauri build check**

Run:

```powershell
bun run tauri build --debug
```

Expected: Build reaches Rust compilation and succeeds. If the command fails because Windows bundling prerequisites are missing, capture the exact error and still run `cd src-tauri; cargo test -- --test-threads=1` as the Rust acceptance gate.

- [ ] **Step 8: Commit Task 3**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/state.rs src-tauri/src/commands
git commit -m "feat: expose local backend health command"
```

## Task 4: TypeScript Domain Services And Local Adapter

**Files:**
- Create: `src/services/types.ts`
- Create: `src/services/ports.ts`
- Create: `src/services/local-adapter.ts`
- Create: `src/services/mock-adapter.ts`
- Create: `src/services/local-adapter.test.ts`

- [ ] **Step 1: Write failing service adapter tests**

Create `src/services/local-adapter.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";

import { createLocalServices } from "./local-adapter";
import { createMockServices } from "./mock-adapter";

describe("local service adapter", () => {
  it("calls the Tauri health command through the injected invoker", async () => {
    const invoke = vi.fn().mockResolvedValue({
      backend: "local",
      appVersion: "0.1.0",
      databasePath: "C:/Users/test/AppData/Roaming/vantumpos/vantumpos.sqlite3",
      migrated: true,
    });

    const services = createLocalServices(invoke);
    const health = await services.settings.getHealth();

    expect(invoke).toHaveBeenCalledWith("get_app_health");
    expect(health.backend).toBe("local");
    expect(health.migrated).toBe(true);
  });
});

describe("mock service adapter", () => {
  it("returns deterministic health for UI tests", async () => {
    const services = createMockServices();
    const health = await services.settings.getHealth();

    expect(health).toEqual({
      backend: "local",
      appVersion: "test",
      databasePath: "mock://vantumpos.sqlite3",
      migrated: true,
    });
  });
});
```

- [ ] **Step 2: Run service tests to verify they fail**

Run:

```powershell
bun run test -- src/services/local-adapter.test.ts
```

Expected: FAIL because service files are missing.

- [ ] **Step 3: Create `src/services/types.ts`**

```ts
export type BackendKind = "local";

export interface AppHealth {
  backend: BackendKind;
  appVersion: string;
  databasePath: string;
  migrated: boolean;
}
```

- [ ] **Step 4: Create `src/services/ports.ts`**

```ts
import type { AppHealth } from "./types";

export interface SettingsService {
  getHealth(): Promise<AppHealth>;
}

export interface PosServices {
  settings: SettingsService;
}
```

- [ ] **Step 5: Create `src/services/local-adapter.ts`**

```ts
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import type { PosServices } from "./ports";
import type { AppHealth } from "./types";

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export function createLocalServices(invoke: InvokeFn = tauriInvoke): PosServices {
  return {
    settings: {
      getHealth: () => invoke<AppHealth>("get_app_health"),
    },
  };
}
```

- [ ] **Step 6: Create `src/services/mock-adapter.ts`**

```ts
import type { PosServices } from "./ports";

export function createMockServices(): PosServices {
  return {
    settings: {
      async getHealth() {
        return {
          backend: "local",
          appVersion: "test",
          databasePath: "mock://vantumpos.sqlite3",
          migrated: true,
        };
      },
    },
  };
}
```

- [ ] **Step 7: Run service tests**

Run:

```powershell
bun run test -- src/services/local-adapter.test.ts
```

Expected: PASS for both adapter tests.

- [ ] **Step 8: Commit Task 4**

```powershell
git add src/services
git commit -m "feat: add local domain service adapter"
```

## Task 5: shadcn Desktop App Shell

**Files:**
- Create: `src/app/navigation.ts`
- Create: `src/app/BackendStatus.tsx`
- Create: `src/app/AppShell.tsx`
- Create: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

- [ ] **Step 1: Confirm shadcn context before UI work**

Run:

```powershell
bunx --bun shadcn@latest info --json
bunx --bun shadcn@latest search '@shadcn' -q "sidebar" --json
bunx --bun shadcn@latest search '@shadcn' -q "dashboard" --json
bunx --bun shadcn@latest search '@shadcn' -q "auth" --json
```

Expected:

- `project.frameworkName` is `vite`.
- `config.style` is `base-mira`.
- `config.iconLibrary` is `lucide`.
- installed components include `sidebar`, `table`, `field`, `input-group`, `badge`, `button`, `skeleton`, and `sonner`.
- registry search includes sidebar blocks and `dashboard-01`.

Run these commands sequentially on Windows. Do not parallelize `bunx --bun shadcn@latest ...` because Bun cache file locks can produce `EBUSY`.

- [ ] **Step 2: Write failing app shell tests**

Create `src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AppShell } from "./app/AppShell";
import { createMockServices } from "./services/mock-adapter";

describe("AppShell", () => {
  it("renders the POS navigation and backend status", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(screen.getByRole("heading", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Artikli" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Lager" })).toBeInTheDocument();
    expect(await screen.findByText("Lokalna baza spremna")).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run app shell test to verify it fails**

Run:

```powershell
bun run test -- src/App.test.tsx
```

Expected: FAIL because `src/app/AppShell.tsx` is missing.

- [ ] **Step 4: Create `src/app/navigation.ts`**

```ts
import {
  ArchiveIcon,
  BarChart3Icon,
  FileSpreadsheetIcon,
  PackageIcon,
  ReceiptTextIcon,
  SettingsIcon,
  ShoppingCartIcon,
  UploadIcon,
} from "lucide-react";

export const navigationItems = [
  {
    id: "register",
    label: "Kasa",
    icon: ShoppingCartIcon,
  },
  {
    id: "products",
    label: "Artikli",
    icon: PackageIcon,
  },
  {
    id: "inventory",
    label: "Lager",
    icon: ArchiveIcon,
  },
  {
    id: "receipts",
    label: "Racuni",
    icon: ReceiptTextIcon,
  },
  {
    id: "reports",
    label: "Izvestaji",
    icon: BarChart3Icon,
  },
  {
    id: "import",
    label: "Import",
    icon: UploadIcon,
  },
  {
    id: "settings",
    label: "Podesavanja",
    icon: SettingsIcon,
  },
] as const;

export type NavigationItemId = (typeof navigationItems)[number]["id"];

export const foundationCards = [
  {
    title: "Lokalna kasa",
    description: "Prodajni tok ce koristiti lokalni SQLite backend.",
    icon: FileSpreadsheetIcon,
  },
  {
    title: "Jedna radnja",
    description: "MVP je za jedan objekat, jednu kasu i jednu bazu.",
    icon: PackageIcon,
  },
  {
    title: "Adapter arhitektura",
    description: "UI komunicira preko domain service ugovora.",
    icon: SettingsIcon,
  },
] as const;
```

- [ ] **Step 5: Create `src/app/BackendStatus.tsx`**

```tsx
import { useEffect, useState } from "react";
import { CheckCircle2Icon, DatabaseIcon, Loader2Icon, XCircleIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { PosServices } from "@/services/ports";
import type { AppHealth } from "@/services/types";

interface BackendStatusProps {
  services: PosServices;
}

type HealthState =
  | { status: "loading" }
  | { status: "ready"; health: AppHealth }
  | { status: "error"; message: string };

export function BackendStatus({ services }: BackendStatusProps) {
  const [state, setState] = useState<HealthState>({ status: "loading" });

  useEffect(() => {
    let active = true;

    services.settings
      .getHealth()
      .then((health) => {
        if (active) {
          setState({ status: "ready", health });
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setState({
            status: "error",
            message: error instanceof Error ? error.message : "Backend nije dostupan.",
          });
        }
      });

    return () => {
      active = false;
    };
  }, [services]);

  if (state.status === "loading") {
    return (
      <Badge variant="outline">
        <Loader2Icon data-icon="inline-start" />
        Provera lokalne baze
      </Badge>
    );
  }

  if (state.status === "error") {
    return (
      <Button variant="destructive" size="sm">
        <XCircleIcon data-icon="inline-start" />
        {state.message}
      </Button>
    );
  }

  return (
    <Badge variant={state.health.migrated ? "secondary" : "destructive"}>
      {state.health.migrated ? (
        <CheckCircle2Icon data-icon="inline-start" />
      ) : (
        <DatabaseIcon data-icon="inline-start" />
      )}
      Lokalna baza spremna
    </Badge>
  );
}
```

- [ ] **Step 6: Create `src/app/AppShell.tsx`**

```tsx
import { useState } from "react";
import { StoreIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import type { PosServices } from "@/services/ports";

import { BackendStatus } from "./BackendStatus";
import { foundationCards, navigationItems, type NavigationItemId } from "./navigation";

interface AppShellProps {
  services: PosServices;
}

export function AppShell({ services }: AppShellProps) {
  const [activeItem, setActiveItem] = useState<NavigationItemId>("register");
  const activeNavigationItem =
    navigationItems.find((item) => item.id === activeItem) ?? navigationItems[0];

  return (
    <SidebarProvider>
      <Sidebar collapsible="icon">
        <SidebarHeader>
          <div className="flex items-center gap-2 px-2 py-1">
            <div className="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
              <StoreIcon />
            </div>
            <div className="min-w-0 group-data-[collapsible=icon]:hidden">
              <div className="truncate text-sm font-semibold">VantumPOS</div>
              <div className="truncate text-xs text-muted-foreground">Lokalna kasa</div>
            </div>
          </div>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupLabel>Rad</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {navigationItems.map((item) => (
                  <SidebarMenuItem key={item.id}>
                    <SidebarMenuButton
                      isActive={item.id === activeItem}
                      tooltip={item.label}
                      onClick={() => setActiveItem(item.id)}
                    >
                      <item.icon />
                      <span>{item.label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarSeparator />
        <SidebarFooter>
          <div className="flex flex-col gap-2 px-2 group-data-[collapsible=icon]:hidden">
            <Badge variant="outline">Smena nije otvorena</Badge>
            <span className="text-xs text-muted-foreground">Admin</span>
          </div>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-12 shrink-0 items-center justify-between gap-3 border-b px-4">
          <div className="flex min-w-0 items-center gap-2">
            <SidebarTrigger />
            <div className="min-w-0">
              <h1 className="truncate text-base font-semibold">{activeNavigationItem.label}</h1>
              <p className="truncate text-xs text-muted-foreground">
                Jedna radnja, jedna kasa, lokalna baza
              </p>
            </div>
          </div>
          <BackendStatus services={services} />
        </header>

        <main className="flex flex-1 flex-col gap-4 p-4">
          <section className="grid gap-3 md:grid-cols-3">
            {foundationCards.map((card) => (
              <div key={card.title} className="rounded-md border bg-card p-4 text-card-foreground">
                <div className="flex items-center gap-2">
                  <card.icon />
                  <h2 className="text-sm font-semibold">{card.title}</h2>
                </div>
                <p className="mt-2 text-xs text-muted-foreground">{card.description}</p>
              </div>
            ))}
          </section>

          <section className="flex min-h-80 flex-1 flex-col rounded-md border bg-card text-card-foreground">
            <div className="border-b p-4">
              <h2 className="text-sm font-semibold">{activeNavigationItem.label}</h2>
              <p className="text-xs text-muted-foreground">
                Ovaj ekran je spreman za sledeci plan implementacije.
              </p>
            </div>
            <div className="flex flex-1 items-center justify-center p-6">
              <Button variant="outline">Spremno za modul</Button>
            </div>
          </section>
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
```

- [ ] **Step 7: Replace `src/App.tsx`**

```tsx
import "./App.css";

import { AppShell } from "@/app/AppShell";
import { createLocalServices } from "@/services/local-adapter";

const services = createLocalServices();

function App() {
  return <AppShell services={services} />;
}

export default App;
```

- [ ] **Step 8: Replace `src/App.css`**

```css
@import "tailwindcss";
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@fontsource-variable/inter";

@custom-variant dark (&:is(.dark *));

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;
  color: #0f0f0f;
  background-color: #f6f6f6;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.145 0 0);
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --secondary: oklch(0.97 0 0);
  --secondary-foreground: oklch(0.205 0 0);
  --muted: oklch(0.97 0 0);
  --muted-foreground: oklch(0.556 0 0);
  --accent: oklch(0.97 0 0);
  --accent-foreground: oklch(0.205 0 0);
  --destructive: oklch(0.577 0.245 27.325);
  --border: oklch(0.922 0 0);
  --input: oklch(0.922 0 0);
  --ring: oklch(0.708 0 0);
  --chart-1: oklch(0.87 0 0);
  --chart-2: oklch(0.556 0 0);
  --chart-3: oklch(0.439 0 0);
  --chart-4: oklch(0.371 0 0);
  --chart-5: oklch(0.269 0 0);
  --radius: 0.625rem;
  --sidebar: oklch(0.985 0 0);
  --sidebar-foreground: oklch(0.145 0 0);
  --sidebar-primary: oklch(0.205 0 0);
  --sidebar-primary-foreground: oklch(0.985 0 0);
  --sidebar-accent: oklch(0.97 0 0);
  --sidebar-accent-foreground: oklch(0.205 0 0);
  --sidebar-border: oklch(0.922 0 0);
  --sidebar-ring: oklch(0.708 0 0);
}

body {
  margin: 0;
}

#root {
  min-height: 100vh;
}

@theme inline {
  --font-heading: var(--font-sans);
  --font-sans: "Inter Variable", sans-serif;
  --color-sidebar-ring: var(--sidebar-ring);
  --color-sidebar-border: var(--sidebar-border);
  --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-primary-foreground: var(--sidebar-primary-foreground);
  --color-sidebar-primary: var(--sidebar-primary);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar: var(--sidebar);
  --color-chart-5: var(--chart-5);
  --color-chart-4: var(--chart-4);
  --color-chart-3: var(--chart-3);
  --color-chart-2: var(--chart-2);
  --color-chart-1: var(--chart-1);
  --color-ring: var(--ring);
  --color-input: var(--input);
  --color-border: var(--border);
  --color-destructive: var(--destructive);
  --color-accent-foreground: var(--accent-foreground);
  --color-accent: var(--accent);
  --color-muted-foreground: var(--muted-foreground);
  --color-muted: var(--muted);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-secondary: var(--secondary);
  --color-primary-foreground: var(--primary-foreground);
  --color-primary: var(--primary);
  --color-popover-foreground: var(--popover-foreground);
  --color-popover: var(--popover);
  --color-card-foreground: var(--card-foreground);
  --color-card: var(--card);
  --color-foreground: var(--foreground);
  --color-background: var(--background);
  --radius-sm: calc(var(--radius) * 0.6);
  --radius-md: calc(var(--radius) * 0.8);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) * 1.4);
  --radius-2xl: calc(var(--radius) * 1.8);
  --radius-3xl: calc(var(--radius) * 2.2);
  --radius-4xl: calc(var(--radius) * 2.6);
}

.dark {
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);
  --card: oklch(0.205 0 0);
  --card-foreground: oklch(0.985 0 0);
  --popover: oklch(0.205 0 0);
  --popover-foreground: oklch(0.985 0 0);
  --primary: oklch(0.922 0 0);
  --primary-foreground: oklch(0.205 0 0);
  --secondary: oklch(0.269 0 0);
  --secondary-foreground: oklch(0.985 0 0);
  --muted: oklch(0.269 0 0);
  --muted-foreground: oklch(0.708 0 0);
  --accent: oklch(0.269 0 0);
  --accent-foreground: oklch(0.985 0 0);
  --destructive: oklch(0.704 0.191 22.216);
  --border: oklch(1 0 0 / 10%);
  --input: oklch(1 0 0 / 15%);
  --ring: oklch(0.556 0 0);
  --chart-1: oklch(0.87 0 0);
  --chart-2: oklch(0.556 0 0);
  --chart-3: oklch(0.439 0 0);
  --chart-4: oklch(0.371 0 0);
  --chart-5: oklch(0.269 0 0);
  --sidebar: oklch(0.205 0 0);
  --sidebar-foreground: oklch(0.985 0 0);
  --sidebar-primary: oklch(0.488 0.243 264.376);
  --sidebar-primary-foreground: oklch(0.985 0 0);
  --sidebar-accent: oklch(0.269 0 0);
  --sidebar-accent-foreground: oklch(0.985 0 0);
  --sidebar-border: oklch(1 0 0 / 10%);
  --sidebar-ring: oklch(0.556 0 0);
}

@layer base {
  * {
    @apply border-border outline-ring/50;
  }

  body {
    @apply bg-background text-foreground;
  }

  html {
    @apply font-sans;
  }

  button:not(:disabled),
  [role="button"]:not(:disabled) {
    cursor: pointer;
  }
}
```

- [ ] **Step 9: Run app shell tests**

Run:

```powershell
bun run test -- src/App.test.tsx src/services/local-adapter.test.ts src/lib/money.test.ts
```

Expected: PASS for app shell, service adapter, and money tests.

- [ ] **Step 10: Run frontend build**

Run:

```powershell
bun run build
```

Expected: PASS.

- [ ] **Step 11: Commit Task 5**

```powershell
git add src/App.tsx src/App.css src/App.test.tsx src/app
git commit -m "feat: add shadcn desktop app shell"
```

## Task 6: End-To-End Foundation Verification

**Files:**
- No planned file changes.

- [ ] **Step 1: Run all frontend tests**

Run:

```powershell
bun run test
```

Expected: PASS.

- [ ] **Step 2: Run frontend build**

Run:

```powershell
bun run build
```

Expected: PASS.

- [ ] **Step 3: Run Rust tests**

Run:

```powershell
cd src-tauri
cargo test -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 4: Run Tauri debug build**

Run:

```powershell
bun run tauri build --debug
```

Expected: PASS. If Windows bundling prerequisites block this command, record the exact failure and keep `bun run build` plus `cargo test -- --test-threads=1` as the verified gates.

- [ ] **Step 5: Inspect final diff**

Run:

```powershell
git status --short
git diff --check
git log --oneline -5
```

Expected:

- `git diff --check` has no output.
- `git status --short` only shows intended files if Task 6 has no final commit yet.
- Recent commits include the task commits from this plan.

- [ ] **Step 6: Commit verification notes only if files changed**

If a verification command required a small fix, commit that fix with:

```powershell
git add <changed-files>
git commit -m "fix: stabilize local foundation verification"
```

If no files changed, do not create an empty commit.

## Acceptance Criteria

The foundation is complete when:

- Frontend tests run with `bun run test`.
- Frontend build passes with `bun run build`.
- Rust tests pass with `cd src-tauri; cargo test -- --test-threads=1`.
- Tauri app initializes a local SQLite database in the app data folder.
- SQLite migrations create all core MVP tables from the approved spec.
- `get_app_health` returns backend kind, app version, database path, and migration status.
- React app no longer shows the Tauri/Vite starter screen.
- App shell uses shadcn `Sidebar` and displays the MVP navigation.
- UI gets backend health through the TypeScript service interface, not direct component-level `invoke`.
- `git diff --check` passes.
