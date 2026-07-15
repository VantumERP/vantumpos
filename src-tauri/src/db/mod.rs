mod migrations;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection};

use crate::app_error::AppError;
use crate::clock::utc_now;
use crate::security::hash_credential;

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
        db.seed_initial_admin()?;
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
PRAGMA journal_mode = WAL;
"#,
        )?;
        connection.create_scalar_function(
            "fold",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(crate::text::fold_text(&ctx.get::<String>(0)?)),
        )?;
        Ok(connection)
    }

    pub fn migrate(&self) -> Result<(), AppError> {
        let mut connection = self.open()?;
        migrations::run_migrations(&mut connection)
    }

    fn seed_initial_admin(&self) -> Result<(), AppError> {
        let connection = self.open()?;
        let user_count: i64 =
            connection.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;

        if user_count > 0 {
            return Ok(());
        }

        let now = utc_now()?;
        let pin_hash = hash_credential("1234")?;

        connection.execute(
            "INSERT INTO users (
                username,
                display_name,
                role,
                pin_hash,
                active,
                created_at,
                updated_at
             )
             VALUES ('admin', 'Administrator', 'admin', ?1, 1, ?2, ?2)",
            params![pin_hash, now],
        )?;

        Ok(())
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
    use rusqlite::{Error, ErrorCode};

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
        "idx_shifts_one_open_per_user",
        "idx_inventory_movements_product",
        "idx_sales_created_at",
        "idx_sales_shift",
        "idx_sales_original_sale",
        "idx_sale_items_product",
        "idx_sale_items_sale",
        "idx_sale_items_original_sale_item",
        "idx_sale_payments_sale",
        "idx_import_job_rows_job",
        "idx_backup_jobs_created_at",
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
    fn auth_shift_migration_adds_login_and_shift_notes() {
        with_test_database("auth_shift_migration_adds_login_and_shift_notes", |db| {
            let connection = db.open().expect("database should open");

            for (table_name, column_name) in [
                ("users", "last_login_at"),
                ("shifts", "opening_note"),
                ("shifts", "closing_note"),
            ] {
                let exists: i64 = connection
                        .query_row(
                            &format!(
                                "SELECT COUNT(*) FROM pragma_table_info('{table_name}') WHERE name = ?1"
                            ),
                            params![column_name],
                            |row| row.get(0),
                        )
                        .expect("column metadata should query");

                assert_eq!(exists, 1, "expected {table_name}.{column_name}");
            }
        });
    }

    #[test]
    fn new_seeds_initial_admin_user() {
        with_test_database("new_seeds_initial_admin_user", |db| {
            let connection = db.open().expect("database should open");

            let admin: (String, String, String, i64) = connection
                .query_row(
                    "SELECT username, display_name, role, active FROM users WHERE username = 'admin'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("bootstrap admin should exist");

            assert_eq!(
                admin,
                ("admin".into(), "Administrator".into(), "admin".into(), 1,),
            );
        });
    }

    #[test]
    fn migration_v6_allows_signed_sale_payment_amounts() {
        with_test_database("migration_v6_signed_sale_payments", |db| {
            let connection = db.open().expect("database should open");
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'sale_payments'",
                    [],
                    |row| row.get(0),
                )
                .expect("sale_payments schema should load");
            assert!(
                schema.contains("amount_minor") && schema.contains("<> 0"),
                "expected signed amount_minor, schema was: {schema}"
            );
            assert!(
                !schema.contains(">= 0"),
                "expected no non-negative constraint, schema was: {schema}"
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

                assert_eq!(migration_count, 6);
            },
        );
    }

    #[test]
    fn catalog_external_source_migration_adds_provenance_columns() {
        with_test_database(
            "catalog_external_source_migration_adds_provenance_columns",
            |db| {
                let connection = db.open().expect("database should open");

                for column_name in [
                    "external_source_provider",
                    "external_source_label",
                    "external_source_barcode",
                    "external_source_fetched_at",
                    "external_source_accepted_fields_json",
                ] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = ?1",
                            params![column_name],
                            |row| row.get(0),
                        )
                        .expect("column metadata should query");

                    assert_eq!(exists, 1, "expected products.{column_name}");
                }
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
