use rusqlite::{params, Connection};

use crate::app_error::AppError;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
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
    opening_cash_minor INTEGER NOT NULL DEFAULT 0 CHECK (opening_cash_minor >= 0),
    expected_cash_minor INTEGER NOT NULL DEFAULT 0 CHECK (expected_cash_minor >= 0),
    counted_cash_minor INTEGER CHECK (counted_cash_minor >= 0),
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
    minimum_stock_milli INTEGER NOT NULL DEFAULT 0 CHECK (minimum_stock_milli >= 0),
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
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli != 0),
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
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor >= 0),
    discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (discount_minor >= 0),
    tax_minor INTEGER NOT NULL CHECK (tax_minor >= 0),
    total_minor INTEGER NOT NULL CHECK (total_minor >= 0),
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
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli != 0),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (discount_minor >= 0),
    tax_rate_basis_points INTEGER NOT NULL CHECK (tax_rate_basis_points >= 0),
    tax_minor INTEGER NOT NULL CHECK (tax_minor >= 0),
    total_minor INTEGER NOT NULL CHECK (total_minor >= 0)
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
    total_rows INTEGER NOT NULL DEFAULT 0 CHECK (total_rows >= 0),
    error_rows INTEGER NOT NULL DEFAULT 0 CHECK (error_rows >= 0),
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
CREATE INDEX idx_inventory_movements_product ON inventory_movements(product_id, created_at);
CREATE INDEX idx_sales_created_at ON sales(created_at);
CREATE INDEX idx_sales_shift ON sales(shift_id);
CREATE INDEX idx_sale_items_product ON sale_items(product_id);
CREATE INDEX idx_sale_items_sale ON sale_items(sale_id);
CREATE INDEX idx_sale_payments_sale ON sale_payments(sale_id);
CREATE INDEX idx_import_job_rows_job ON import_job_rows(import_job_id);
"#,
    },
    Migration {
        version: 2,
        name: "backup_job_metadata",
        sql: r#"
CREATE UNIQUE INDEX idx_shifts_one_open_per_user
ON shifts(user_id)
WHERE status = 'open';

CREATE TABLE backup_jobs_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_type TEXT NOT NULL CHECK (backup_type IN ('manual', 'automatic', 'restore', 'pre_restore')),
    path TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    error_message TEXT,
    file_size_bytes INTEGER CHECK (file_size_bytes IS NULL OR file_size_bytes >= 0),
    created_at TEXT NOT NULL,
    completed_at TEXT
);

INSERT INTO backup_jobs_next (
    id,
    backup_type,
    path,
    status,
    error_message,
    created_at,
    completed_at
)
SELECT
    id,
    backup_type,
    path,
    status,
    error_message,
    created_at,
    CASE WHEN status = 'completed' THEN created_at ELSE NULL END
FROM backup_jobs;

DROP TABLE backup_jobs;
ALTER TABLE backup_jobs_next RENAME TO backup_jobs;
CREATE INDEX idx_backup_jobs_created_at ON backup_jobs(created_at);
"#,
    },
    Migration {
        version: 3,
        name: "auth_users_shifts_session_fields",
        sql: r#"
ALTER TABLE users ADD COLUMN last_login_at TEXT;
ALTER TABLE shifts ADD COLUMN opening_note TEXT;
ALTER TABLE shifts ADD COLUMN closing_note TEXT;
"#,
    },
    Migration {
        version: 4,
        name: "receipt_documents_and_returns",
        sql: r#"
ALTER TABLE sales ADD COLUMN document_type TEXT NOT NULL DEFAULT 'sale' CHECK (document_type IN ('sale', 'void', 'return'));
ALTER TABLE sales ADD COLUMN void_reason TEXT;
ALTER TABLE sales ADD COLUMN return_reason TEXT;
ALTER TABLE sale_items ADD COLUMN original_sale_item_id INTEGER REFERENCES sale_items(id);

CREATE INDEX idx_sales_original_sale ON sales(original_sale_id);
CREATE INDEX idx_sale_items_original_sale_item ON sale_items(original_sale_item_id);
"#,
    },
    Migration {
        version: 5,
        name: "product_external_source_provenance",
        sql: r#"
ALTER TABLE products ADD COLUMN external_source_provider TEXT;
ALTER TABLE products ADD COLUMN external_source_label TEXT;
ALTER TABLE products ADD COLUMN external_source_barcode TEXT;
ALTER TABLE products ADD COLUMN external_source_fetched_at TEXT;
ALTER TABLE products ADD COLUMN external_source_accepted_fields_json TEXT;
"#,
    },
];

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_database_path;

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info should prepare");
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("table_info should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("table_info rows should collect");
        names.iter().any(|name| name == column)
    }

    #[test]
    fn old_database_migrates_forward_to_latest_schema() {
        let path = test_database_path("migrations_forward");

        {
            let mut conn = Connection::open(&path).expect("connection should open");

            // Simulate an installed database stuck at schema version 1 only.
            conn.execute_batch(
                r#"
CREATE TABLE _migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
"#,
            )
            .expect("migrations table should create");
            conn.execute_batch(MIGRATIONS[0].sql)
                .expect("version 1 schema should apply");
            conn.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                params![MIGRATIONS[0].version, MIGRATIONS[0].name],
            )
            .expect("version 1 should record");

            // Columns added by later migrations must be absent before the upgrade.
            assert!(!column_exists(&conn, "users", "last_login_at"));
            assert!(!column_exists(&conn, "sales", "document_type"));
            assert!(!column_exists(
                &conn,
                "products",
                "external_source_provider"
            ));

            // The real installed-base upgrade path.
            run_migrations(&mut conn).expect("forward migration should succeed");

            let applied: i64 = conn
                .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                .expect("migration count should query");
            assert_eq!(applied, MIGRATIONS.len() as i64);

            assert!(column_exists(&conn, "users", "last_login_at"));
            assert!(column_exists(&conn, "sales", "document_type"));
            assert!(column_exists(&conn, "products", "external_source_provider"));

            // Re-running migrations on an up-to-date database is a no-op.
            run_migrations(&mut conn).expect("re-running migrations should be a no-op");
            let applied_again: i64 = conn
                .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                .expect("migration count should query");
            assert_eq!(applied_again, MIGRATIONS.len() as i64);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
