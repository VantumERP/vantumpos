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
    pub fn new(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
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
        let connection = Connection::open(self.path.as_path())?;
        connection.execute_batch(
            r#"
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
"#,
        )?;
        Ok(connection)
    }

    pub fn migrate(&self) -> Result<(), AppError> {
        let mut connection = self.open()?;
        migrations::run_migrations(&mut connection)
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
    use rusqlite::{params, Error, ErrorCode};

    const CORE_TABLES: &[&str] = &[
        "settings",
        "users",
        "shifts",
        "categories",
        "tax_rates",
        "products",
        "inventory_balances",
        "inventory_movements",
        "sales",
        "sale_items",
        "sale_payments",
        "import_jobs",
        "import_job_rows",
        "backup_jobs",
    ];

    const EXPLICIT_INDEXES: &[&str] = &[
        "idx_products_name",
        "idx_inventory_movements_product",
        "idx_sales_created_at",
        "idx_sales_shift",
        "idx_sale_items_product",
        "idx_sale_items_sale",
        "idx_sale_payments_sale",
        "idx_import_job_rows_job",
    ];

    fn schema_object_exists(connection: &Connection, object_type: &str, name: &str) -> bool {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
                params![object_type, name],
                |row| row.get(0),
            )
            .expect("schema object count should query");

        count == 1
    }

    fn assert_sqlite_constraint_failure(result: rusqlite::Result<usize>) {
        match result {
            Err(Error::SqliteFailure(error, _)) if error.code == ErrorCode::ConstraintViolation => {
            }
            other => panic!("expected SQLite constraint failure, got {other:?}"),
        }
    }

    fn seed_shift(connection: &Connection) -> (i64, i64) {
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('cashier', 'Cashier', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let user_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO shifts (
                    user_id,
                    opened_at,
                    opening_cash_minor,
                    expected_cash_minor,
                    status,
                    created_at,
                    updated_at
                 )
                 VALUES (?1, '2026-01-01T00:00:00Z', 0, 0, 'open', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![user_id],
            )
            .expect("shift should insert");
        let shift_id = connection.last_insert_rowid();

        (shift_id, user_id)
    }

    fn seed_sale(connection: &Connection) -> i64 {
        let (shift_id, cashier_id) = seed_shift(connection);

        connection
            .execute(
                "INSERT INTO sales (
                    local_receipt_number,
                    shift_id,
                    cashier_id,
                    status,
                    fiscal_status,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    total_minor,
                    created_at,
                    updated_at
                 )
                 VALUES ('R-VALID', ?1, ?2, 'completed', 'not_fiscalized', 1000, 0, 200, 1200, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![shift_id, cashier_id],
            )
            .expect("sale should insert");

        connection.last_insert_rowid()
    }

    #[test]
    fn new_initializes_and_migrates_empty_database() {
        let path = test_database_path("new_initializes_and_migrates_empty_database");

        let db = Db::new(&path).expect("database should initialize");
        let connection = db.open().expect("database should open");

        for table_name in CORE_TABLES {
            assert!(
                schema_object_exists(&connection, "table", table_name),
                "expected table {table_name} to exist"
            );
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn new_creates_planned_explicit_indexes() {
        let path = test_database_path("new_creates_planned_explicit_indexes");

        let db = Db::new(&path).expect("database should initialize");
        let connection = db.open().expect("database should open");

        for index_name in EXPLICIT_INDEXES {
            assert!(
                schema_object_exists(&connection, "index", index_name),
                "expected explicit index {index_name} to exist"
            );
        }

        assert!(
            !schema_object_exists(&connection, "index", "idx_products_barcode"),
            "barcode UNIQUE constraint already creates an implicit index"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sales_reject_negative_money_fields() {
        let path = test_database_path("sales_reject_negative_money_fields");

        let db = Db::new(&path).expect("database should initialize");
        let connection = db.open().expect("database should open");
        let (shift_id, cashier_id) = seed_shift(&connection);

        for column_name in [
            "subtotal_minor",
            "discount_minor",
            "tax_minor",
            "total_minor",
        ] {
            let mut subtotal_minor = 1000;
            let mut discount_minor = 0;
            let mut tax_minor = 200;
            let mut total_minor = 1200;

            match column_name {
                "subtotal_minor" => subtotal_minor = -1,
                "discount_minor" => discount_minor = -1,
                "tax_minor" => tax_minor = -1,
                "total_minor" => total_minor = -1,
                _ => unreachable!("unexpected sales money column"),
            }

            let result = connection.execute(
                "INSERT INTO sales (
                    local_receipt_number,
                    shift_id,
                    cashier_id,
                    status,
                    fiscal_status,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    total_minor,
                    created_at,
                    updated_at
                 )
                 VALUES (?1, ?2, ?3, 'completed', 'not_fiscalized', ?4, ?5, ?6, ?7, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![
                    format!("R-INVALID-{column_name}"),
                    shift_id,
                    cashier_id,
                    subtotal_minor,
                    discount_minor,
                    tax_minor,
                    total_minor
                ],
            );

            assert_sqlite_constraint_failure(result);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sale_items_reject_zero_quantity_and_negative_money_fields() {
        let path = test_database_path("sale_items_reject_zero_quantity_and_negative_money_fields");

        let db = Db::new(&path).expect("database should initialize");
        let connection = db.open().expect("database should open");
        let sale_id = seed_sale(&connection);

        for (column_name, quantity_milli) in [
            ("quantity_milli", 0),
            ("unit_price_minor", 1000),
            ("discount_minor", 1000),
            ("tax_rate_basis_points", 1000),
            ("tax_minor", 1000),
            ("total_minor", 1000),
        ] {
            let mut unit_price_minor = 1000;
            let mut discount_minor = 0;
            let mut tax_rate_basis_points = 2000;
            let mut tax_minor = 200;
            let mut total_minor = 1200;

            match column_name {
                "quantity_milli" => {}
                "unit_price_minor" => unit_price_minor = -1,
                "discount_minor" => discount_minor = -1,
                "tax_rate_basis_points" => tax_rate_basis_points = -1,
                "tax_minor" => tax_minor = -1,
                "total_minor" => total_minor = -1,
                _ => unreachable!("unexpected sale item money column"),
            }

            let result = connection.execute(
                "INSERT INTO sale_items (
                    sale_id,
                    product_name,
                    product_sku,
                    quantity_milli,
                    unit_price_minor,
                    discount_minor,
                    tax_rate_basis_points,
                    tax_minor,
                    total_minor
                 )
                 VALUES (?1, 'Product', 'SKU-1', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    sale_id,
                    quantity_milli,
                    unit_price_minor,
                    discount_minor,
                    tax_rate_basis_points,
                    tax_minor,
                    total_minor
                ],
            );

            assert_sqlite_constraint_failure(result);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrate_is_idempotent_and_records_initial_migration_once() {
        let path = test_database_path("migrate_is_idempotent_and_records_initial_migration_once");

        let db = Db::new(&path).expect("database should initialize");
        db.migrate().expect("second migration should succeed");

        let connection = db.open().expect("database should open");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("migration count should query");

        assert_eq!(migration_count, 1);

        let _ = std::fs::remove_file(path);
    }
}
