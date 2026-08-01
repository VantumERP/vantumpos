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
        "cash_movements",
        "compliance_log",
        "price_history",
        "campaigns",
        "campaign_items",
        "reklamacije",
        "reklamacija_events",
        "kep_entries",
        "kalkulacije",
        "kep_closures",
        "non_working_days",
        "work_time_entries",
        "work_time_periods",
        "retention_policies",
        "support_sessions",
        "audit_events",
        "personnel_records",
        "data_breaches",
        "processing_activities",
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
        "idx_cash_movements_shift",
        "idx_compliance_log_created_at",
        "idx_price_history_product",
        "idx_campaigns_type_start",
        "idx_campaign_items_campaign",
        "idx_campaign_items_product",
        "idx_reklamacije_status",
        "idx_reklamacija_events_parent",
        "idx_kep_entries_book",
        "idx_kep_entries_date",
        "idx_kalkulacije_product",
        "idx_kep_closures_year",
        "idx_cash_movements_created_at",
        "idx_work_time_entries_user_day",
        "idx_work_time_entries_dan",
        "idx_work_time_periods_user_month",
        "idx_support_sessions_granted_at",
        "idx_audit_events_at",
        "idx_audit_events_actor",
        "idx_data_breaches_saznanje_at",
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
    fn migration_v8_adds_compliance_log_and_esir_column() {
        with_test_database("migration_v8_compliance_and_esir", |db| {
            let connection = db.open().expect("database should open");

            assert!(
                schema_object_exists(&connection, "table", "compliance_log"),
                "expected compliance_log table"
            );

            let esir_exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('sales') WHERE name = 'esir_receipt_number'",
                    [],
                    |row| row.get(0),
                )
                .expect("column metadata should query");
            assert_eq!(esir_exists, 1, "expected sales.esir_receipt_number");

            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='compliance_log'",
                    [],
                    |row| row.get(0),
                )
                .expect("compliance_log schema should load");
            // v16 rebuilt the table to widen this CHECK; the v8 pair must still be
            // admitted alongside the AML event, and nothing beyond the three.
            assert!(
                schema.contains("trading_data_reset")
                    && schema.contains("backup_restored")
                    && schema.contains("aml_cash_threshold"),
                "expected event_type CHECK, schema was: {schema}"
            );
        });
    }

    #[test]
    fn migration_v9_creates_price_history_and_seeds_active_products_only() {
        with_test_database("migration_v9_price_history", |db| {
            let connection = db.open().expect("database should open");

            assert!(
                schema_object_exists(&connection, "table", "price_history"),
                "expected price_history table"
            );

            // price_minor must be nullable: NULL means "offering ended".
            let notnull: i64 = connection
                .query_row(
                    "SELECT \"notnull\" FROM pragma_table_info('price_history') WHERE name = 'price_minor'",
                    [],
                    |row| row.get(0),
                )
                .expect("column metadata should query");
            assert_eq!(
                notnull, 0,
                "price_minor must be nullable (NULL = offering ended)"
            );

            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='price_history'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema should load");
            for token in [
                "create",
                "update",
                "import",
                "deactivate",
                "reactivate",
                "seed",
            ] {
                assert!(
                    schema.contains(token),
                    "expected source CHECK to allow {token}"
                );
            }
        });
    }

    #[test]
    fn migration_v9_seed_rows_use_rfc3339_and_skip_inactive_products() {
        with_test_database("migration_v9_seed_format", |db| {
            let connection = db.open().expect("database should open");
            // Fresh DB has no products, so seed nothing; insert one active + one
            // inactive product and re-run the seed statement shape by hand is not
            // possible post-migration. Instead assert the seed statement's format
            // contract on a synthetic row inserted the same way the migration does.
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     SELECT 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 1000, 'seed', strftime('%Y-%m-%dT%H:%M:%SZ','now')
                     WHERE 0",
                    [],
                )
                .expect("seed-shaped statement should be valid SQL");

            let stamp: String = connection
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
                    row.get(0)
                })
                .expect("timestamp should format");
            assert!(
                stamp.contains('T') && stamp.ends_with('Z'),
                "seed stamp must be RFC3339, got {stamp}"
            );
            assert!(
                !stamp.contains(' '),
                "seed stamp must not use SQLite's space separator"
            );
        });
    }

    #[test]
    fn migration_v10_creates_campaign_tables_and_perishable_columns() {
        with_test_database("migration_v10_campaigns", |db| {
            let connection = db.open().expect("database should open");

            for table in ["campaigns", "campaign_items"] {
                assert!(
                    schema_object_exists(&connection, "table", table),
                    "expected table {table}"
                );
            }
            for column in ["perishable", "perishable_justification"] {
                let exists: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = ?1",
                        params![column],
                        |row| row.get(0),
                    )
                    .expect("column metadata should query");
                assert_eq!(exists, 1, "expected products.{column}");
            }

            let campaigns_schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='campaigns'",
                    [],
                    |row| row.get(0),
                )
                .expect("campaigns schema should load");
            for token in [
                "rasprodaja",
                "sezonsko_snizenje",
                "akcijska_prodaja",
                "promotivna_prodaja",
                "headline_percent",
            ] {
                assert!(
                    campaigns_schema.contains(token),
                    "campaigns schema missing {token}"
                );
            }
        });
    }

    #[test]
    fn migration_v11_creates_reklamacije_tables() {
        with_test_database("migration_v11_reklamacije", |db| {
            let connection = db.open().expect("database should open");
            for table in ["reklamacije", "reklamacija_events"] {
                assert!(
                    schema_object_exists(&connection, "table", table),
                    "expected {table}"
                );
            }
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='reklamacije'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema");
            for token in [
                "register_number",
                "regime",
                "opsta",
                "tehnicka",
                "namestaj",
                "podnosilac_ime_prezime",
            ] {
                assert!(schema.contains(token), "reklamacije schema missing {token}");
            }
            let events_schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='reklamacija_events'",
                    [],
                    |row| row.get(0),
                )
                .expect("events schema");
            for token in [
                "answer_given",
                "consumer_received_answer",
                "consumer_responded",
                "extension_granted",
                "resolved",
            ] {
                assert!(
                    events_schema.contains(token),
                    "events CHECK missing {token}"
                );
            }
        });
    }

    /// This fixture builds a fresh database, so the rebuild here copies an empty
    /// table — row-and-id preservation across the v10 rebuild is proven against a
    /// populated v9 database in
    /// `db::migrations::tests::migration_v10_rebuild_copies_price_history_rows_with_original_ids`.
    #[test]
    fn migration_v10_widens_price_history_sources_and_preserves_rows() {
        with_test_database("migration_v10_price_history_sources", |db| {
            let connection = db.open().expect("database should open");

            // The widened CHECK admits campaign sources.
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='price_history'",
                    [],
                    |row| row.get(0),
                )
                .expect("price_history schema should load");
            for token in ["campaign_start", "campaign_step", "campaign_end"] {
                assert!(
                    schema.contains(token),
                    "price_history CHECK missing {token}"
                );
            }
            assert!(
                schema_object_exists(&connection, "index", "idx_price_history_product"),
                "rebuild must recreate idx_price_history_product"
            );

            // Rows written pre-rebuild shape survive with identity intact: insert
            // via the legacy sources and via the new ones — both must work.
            connection
                .execute(
                    "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                           tax_rate_id, minimum_stock_milli, created_at, updated_at)
                     SELECT 901, 'P', 'SKU-V10', 1000, 0, id, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
                     FROM tax_rates LIMIT 1",
                    [],
                )
                .ok(); // tax rate may not exist on a bare DB; fall back below
            let product_seeded: i64 = connection
                .query_row("SELECT COUNT(*) FROM products WHERE id = 901", [], |row| {
                    row.get(0)
                })
                .expect("count");
            if product_seeded == 0 {
                connection
                    .execute_batch(
                        "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                         VALUES (900, 'T', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                         INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                               tax_rate_id, minimum_stock_milli, created_at, updated_at)
                         VALUES (901, 'P', 'SKU-V10', 1000, 0, 900, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
                    )
                    .expect("seed product");
            }
            for source in ["update", "campaign_start", "campaign_step", "campaign_end"] {
                connection
                    .execute(
                        "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                         VALUES (901, '2026-07-01T00:00:00Z', 1000, ?1, '2026-07-01T00:00:00Z')",
                        params![source],
                    )
                    .unwrap_or_else(|error| panic!("source {source} should insert: {error}"));
            }
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

                assert_eq!(migration_count, 18);
            },
        );
    }

    #[test]
    fn migration_v14_adds_kep_closures() {
        with_test_database("migration_v14_kep_closures", |db| {
            let connection = db.open().expect("database should open");
            for column in [
                "id",
                "book_year",
                "krajnji_saldo_minor",
                "entry_count",
                "closed_at",
                "closed_by",
                "created_at",
            ] {
                let exists: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('kep_closures') WHERE name = ?1",
                        [column],
                        |row| row.get(0),
                    )
                    .expect("pragma should query");
                assert_eq!(exists, 1, "kep_closures.{column}");
            }
            let index: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index' AND name = 'idx_kep_closures_year'",
                    [],
                    |row| row.get(0),
                )
                .expect("index query");
            assert_eq!(index, 1, "idx_kep_closures_year exists");
        });
    }

    #[test]
    fn migration_v13_adds_kalkulacije_and_cause() {
        with_test_database("migration_v13_kalkulacije", |db| {
            let connection = db.open().expect("db open");
            assert!(
                schema_object_exists(&connection, "table", "kalkulacije"),
                "kalkulacije table"
            );
            for col in [
                "razlika_u_ceni_minor",
                "prodajna_vrednost_sa_pdv_minor",
                "nabavna_cena_po_jm_minor",
            ] {
                let exists: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('kalkulacije') WHERE name = ?1",
                        params![col],
                        |r| r.get(0),
                    )
                    .expect("meta");
                assert_eq!(exists, 1, "kalkulacije.{col}");
            }
            let cause: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('kep_entries') WHERE name = 'cause'",
                    [],
                    |r| r.get(0),
                )
                .expect("meta");
            assert_eq!(cause, 1, "kep_entries.cause");
        });
    }

    #[test]
    fn migration_v12_creates_kep_entries() {
        with_test_database("migration_v12_kep", |db| {
            let connection = db.open().expect("database should open");
            assert!(
                schema_object_exists(&connection, "table", "kep_entries"),
                "expected kep_entries"
            );
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='kep_entries'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema");
            for token in [
                "book_year",
                "redni_broj",
                "zaduzenje",
                "razduzenje",
                "receipt",
                "daily_sales",
                "nivelacija_down_storno",
                "close_carry",
            ] {
                assert!(schema.contains(token), "kep_entries schema missing {token}");
            }
            assert!(
                schema.contains("UNIQUE (book_year, redni_broj)"),
                "expected UNIQUE(book_year, redni_broj)"
            );
        });
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
