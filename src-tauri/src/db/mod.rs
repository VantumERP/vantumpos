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

    fn with_test_database(test_name: &str, test: impl FnOnce(&Db)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            test(&db);
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
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

    fn seed_product(connection: &Connection) -> i64 {
        connection
            .execute(
                "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES ('VAT 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        let tax_rate_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO products (
                    name,
                    sku,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    created_at,
                    updated_at
                 )
                 VALUES ('Product', 'SKU-VALID', 1000, 700, ?1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![tax_rate_id],
            )
            .expect("product should insert");

        connection.last_insert_rowid()
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
        with_test_database("new_initializes_and_migrates_empty_database", |db| {
            let connection = db.open().expect("database should open");

            for table_name in CORE_TABLES {
                assert!(
                    schema_object_exists(&connection, "table", table_name),
                    "expected table {table_name} to exist"
                );
            }
        });
    }

    #[test]
    fn new_creates_planned_explicit_indexes() {
        with_test_database("new_creates_planned_explicit_indexes", |db| {
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
        });
    }

    #[test]
    fn sales_reject_negative_money_fields() {
        with_test_database("sales_reject_negative_money_fields", |db| {
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
        });
    }

    #[test]
    fn sale_items_reject_zero_quantity_and_negative_money_fields() {
        with_test_database(
            "sale_items_reject_zero_quantity_and_negative_money_fields",
            |db| {
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
            },
        );
    }

    #[test]
    fn migrate_is_idempotent_and_records_initial_migration_once() {
        with_test_database(
            "migrate_is_idempotent_and_records_initial_migration_once",
            |db| {
                db.migrate().expect("second migration should succeed");

                let connection = db.open().expect("database should open");
                let migration_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                    .expect("migration count should query");

                assert_eq!(migration_count, 1);
            },
        );
    }

    #[test]
    fn open_enables_connection_guarantees_and_foreign_key_enforcement() {
        with_test_database(
            "open_enables_connection_guarantees_and_foreign_key_enforcement",
            |db| {
                let connection = db.open().expect("database should open");

                let foreign_keys: i64 = connection
                    .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                    .expect("foreign_keys pragma should query");
                let busy_timeout: i64 = connection
                    .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
                    .expect("busy_timeout pragma should query");

                assert_eq!(foreign_keys, 1);
                assert_eq!(busy_timeout, 5000);

                let orphan_shift = connection.execute(
                    "INSERT INTO shifts (
                        user_id,
                        opened_at,
                        opening_cash_minor,
                        expected_cash_minor,
                        status,
                        created_at,
                        updated_at
                     )
                     VALUES (9999, '2026-01-01T00:00:00Z', 0, 0, 'open', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    [],
                );

                assert_sqlite_constraint_failure(orphan_shift);
            },
        );
    }

    #[test]
    fn deleting_product_cascades_to_inventory_balance() {
        with_test_database("deleting_product_cascades_to_inventory_balance", |db| {
            let connection = db.open().expect("database should open");
            let product_id = seed_product(&connection);

            connection
                .execute(
                    "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                     VALUES (?1, 1000, '2026-01-01T00:00:00Z')",
                    params![product_id],
                )
                .expect("inventory balance should insert");

            connection
                .execute("DELETE FROM products WHERE id = ?1", params![product_id])
                .expect("product should delete");

            let balance_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM inventory_balances WHERE product_id = ?1",
                    params![product_id],
                    |row| row.get(0),
                )
                .expect("inventory balance count should query");

            assert_eq!(balance_count, 0);
        });
    }

    #[test]
    fn shifts_reject_negative_cash_fields() {
        with_test_database("shifts_reject_negative_cash_fields", |db| {
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO users (username, display_name, role, created_at, updated_at)
                     VALUES ('cashier', 'Cashier', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    [],
                )
                .expect("cashier should insert");
            let user_id = connection.last_insert_rowid();

            for column_name in [
                "opening_cash_minor",
                "expected_cash_minor",
                "counted_cash_minor",
            ] {
                let mut opening_cash_minor = 0;
                let mut expected_cash_minor = 0;
                let mut counted_cash_minor = Some(0);

                match column_name {
                    "opening_cash_minor" => opening_cash_minor = -1,
                    "expected_cash_minor" => expected_cash_minor = -1,
                    "counted_cash_minor" => counted_cash_minor = Some(-1),
                    _ => unreachable!("unexpected shift cash column"),
                }

                let result = connection.execute(
                    "INSERT INTO shifts (
                        user_id,
                        opened_at,
                        opening_cash_minor,
                        expected_cash_minor,
                        counted_cash_minor,
                        status,
                        created_at,
                        updated_at
                     )
                     VALUES (?1, '2026-01-01T00:00:00Z', ?2, ?3, ?4, 'open', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    params![
                        user_id,
                        opening_cash_minor,
                        expected_cash_minor,
                        counted_cash_minor
                    ],
                );

                assert_sqlite_constraint_failure(result);
            }
        });
    }

    #[test]
    fn products_reject_negative_minimum_stock() {
        with_test_database("products_reject_negative_minimum_stock", |db| {
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                     VALUES ('VAT 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    [],
                )
                .expect("tax rate should insert");
            let tax_rate_id = connection.last_insert_rowid();

            let result = connection.execute(
                "INSERT INTO products (
                    name,
                    sku,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    created_at,
                    updated_at
                 )
                 VALUES ('Product', 'SKU-NEGATIVE-STOCK', 1000, 700, ?1, -1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![tax_rate_id],
            );

            assert_sqlite_constraint_failure(result);
        });
    }

    #[test]
    fn inventory_movements_reject_zero_quantity() {
        with_test_database("inventory_movements_reject_zero_quantity", |db| {
            let connection = db.open().expect("database should open");
            let product_id = seed_product(&connection);

            let result = connection.execute(
                "INSERT INTO inventory_movements (
                    product_id,
                    movement_type,
                    quantity_milli,
                    created_at
                 )
                 VALUES (?1, 'receive', 0, '2026-01-01T00:00:00Z')",
                params![product_id],
            );

            assert_sqlite_constraint_failure(result);
        });
    }

    #[test]
    fn import_jobs_reject_negative_row_counts() {
        with_test_database("import_jobs_reject_negative_row_counts", |db| {
            let connection = db.open().expect("database should open");

            for column_name in ["total_rows", "error_rows"] {
                let mut total_rows = 0;
                let mut error_rows = 0;

                match column_name {
                    "total_rows" => total_rows = -1,
                    "error_rows" => error_rows = -1,
                    _ => unreachable!("unexpected import job row count column"),
                }

                let result = connection.execute(
                    "INSERT INTO import_jobs (
                        import_type,
                        file_name,
                        column_mapping_json,
                        status,
                        total_rows,
                        error_rows,
                        created_at
                     )
                     VALUES ('products', ?1, '{}', 'draft', ?2, ?3, '2026-01-01T00:00:00Z')",
                    params![format!("invalid-{column_name}.csv"), total_rows, error_rows],
                );

                assert_sqlite_constraint_failure(result);
            }
        });
    }
}
