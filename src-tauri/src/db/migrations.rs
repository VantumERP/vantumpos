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
    Migration {
        version: 6,
        name: "signed_sale_payments",
        sql: r#"
CREATE TABLE sale_payments_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'card')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor <> 0),
    created_at TEXT NOT NULL
);

INSERT INTO sale_payments_next (id, sale_id, payment_method, amount_minor, created_at)
SELECT id, sale_id, payment_method, amount_minor, created_at FROM sale_payments;

DROP TABLE sale_payments;
ALTER TABLE sale_payments_next RENAME TO sale_payments;
CREATE INDEX idx_sale_payments_sale ON sale_payments(sale_id);
"#,
    },
    Migration {
        version: 7,
        name: "cash_movements",
        sql: r#"
CREATE TABLE cash_movements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shift_id INTEGER NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    movement_type TEXT NOT NULL CHECK (movement_type IN ('pay_in', 'pay_out')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    reason TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_cash_movements_shift ON cash_movements(shift_id);
"#,
    },
    Migration {
        version: 8,
        name: "compliance_log_and_esir_receipt_number",
        sql: r#"
CREATE TABLE compliance_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('trading_data_reset', 'backup_restored')),
    detail_json TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_compliance_log_created_at ON compliance_log(created_at);

ALTER TABLE sales ADD COLUMN esir_receipt_number TEXT;
"#,
    },
    Migration {
        version: 9,
        name: "price_history",
        sql: r#"
CREATE TABLE price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    source TEXT NOT NULL CHECK (source IN ('create', 'update', 'import', 'deactivate', 'reactivate', 'seed')),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_price_history_product ON price_history(product_id, effective_from);

INSERT INTO price_history (product_id, effective_from, price_minor, source, user_id, created_at)
SELECT id,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
       sale_price_minor,
       'seed',
       NULL,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM products
WHERE active = 1;
"#,
    },
    Migration {
        version: 10,
        name: "campaigns_and_price_history_campaign_sources",
        sql: r#"
CREATE TABLE campaigns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_type TEXT NOT NULL CHECK (campaign_type IN ('rasprodaja', 'sezonsko_snizenje', 'akcijska_prodaja', 'promotivna_prodaja')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'ended', 'cancelled')),
    starts_on TEXT NOT NULL,
    ends_on TEXT,
    display_mode TEXT NOT NULL DEFAULT 'two_prices' CHECK (display_mode IN ('two_prices', 'percentage')),
    headline_percent INTEGER CHECK (headline_percent IS NULL OR (headline_percent BETWEEN 1 AND 99)),
    rasprodaja_ground TEXT CHECK (rasprodaja_ground IN ('prestanak_poslovanja', 'prestanak_u_objektu', 'prestanak_prodaje_robe')),
    special_conditions TEXT,
    reduced_utility_reason TEXT,
    marketing_label TEXT,
    season_attested INTEGER NOT NULL DEFAULT 0 CHECK (season_attested IN (0, 1)),
    separation_attested INTEGER NOT NULL DEFAULT 0 CHECK (separation_attested IN (0, 1)),
    activated_at TEXT,
    ended_at TEXT,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_campaigns_type_start ON campaigns(campaign_type, starts_on);

CREATE TABLE campaign_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    campaign_price_minor INTEGER NOT NULL CHECK (campaign_price_minor >= 0),
    prethodna_cena_minor INTEGER CHECK (prethodna_cena_minor IS NULL OR prethodna_cena_minor >= 0),
    anchor_status TEXT NOT NULL CHECK (anchor_status IN ('computed', 'manual', 'none')),
    anchor_window_days INTEGER,
    anchor_truncated INTEGER NOT NULL DEFAULT 0 CHECK (anchor_truncated IN (0, 1)),
    anchor_reason TEXT,
    anchor_justification TEXT,
    future_regular_price_minor INTEGER CHECK (future_regular_price_minor IS NULL OR future_regular_price_minor >= 0),
    pre_campaign_price_minor INTEGER,
    UNIQUE (campaign_id, product_id)
);
CREATE INDEX idx_campaign_items_campaign ON campaign_items(campaign_id);
CREATE INDEX idx_campaign_items_product ON campaign_items(product_id);

ALTER TABLE products ADD COLUMN perishable INTEGER NOT NULL DEFAULT 0 CHECK (perishable IN (0, 1));
ALTER TABLE products ADD COLUMN perishable_justification TEXT;

CREATE TABLE price_history_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    source TEXT NOT NULL CHECK (source IN ('create', 'update', 'import', 'deactivate', 'reactivate', 'seed', 'campaign_start', 'campaign_step', 'campaign_end')),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);

INSERT INTO price_history_next (id, product_id, effective_from, price_minor, source, user_id, created_at)
SELECT id, product_id, effective_from, price_minor, source, user_id, created_at FROM price_history;

DROP TABLE price_history;
ALTER TABLE price_history_next RENAME TO price_history;
CREATE INDEX idx_price_history_product ON price_history(product_id, effective_from);
"#,
    },
    Migration {
        version: 11,
        name: "reklamacije_register",
        sql: r#"
CREATE TABLE reklamacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    register_number INTEGER NOT NULL UNIQUE,
    regime TEXT NOT NULL CHECK (regime IN ('old', 'new')),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'answered', 'awaiting_consumer', 'impasse', 'resolved')),
    filed_at TEXT NOT NULL,
    podnosilac_ime_prezime TEXT NOT NULL,
    kontakt TEXT,
    podaci_o_robi TEXT NOT NULL,
    opis_nesaobraznosti TEXT NOT NULL,
    zahtev TEXT NOT NULL,
    roba_kind TEXT NOT NULL DEFAULT 'opsta' CHECK (roba_kind IN ('opsta', 'tehnicka', 'namestaj')),
    datum_izdavanja_potvrde TEXT NOT NULL,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_reklamacije_status ON reklamacije(status, filed_at);

CREATE TABLE reklamacija_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    reklamacija_id INTEGER NOT NULL REFERENCES reklamacije(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN
        ('answer_given', 'consumer_received_answer', 'consumer_responded', 'extension_granted', 'resolved')),
    event_date TEXT NOT NULL,
    detail_json TEXT,
    consumer_consent INTEGER NOT NULL DEFAULT 0 CHECK (consumer_consent IN (0, 1)),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_reklamacija_events_parent ON reklamacija_events(reklamacija_id, event_date);
"#,
    },
    Migration {
        version: 12,
        name: "kep_entries",
        sql: r#"
CREATE TABLE kep_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL,
    redni_broj INTEGER NOT NULL,
    entry_date TEXT NOT NULL,
    document_date TEXT,
    opis TEXT NOT NULL,
    kolona TEXT NOT NULL CHECK (kolona IN ('zaduzenje', 'razduzenje')),
    amount_minor INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'opening', 'receipt', 'daily_sales',
        'nivelacija_up', 'nivelacija_down_storno',
        'supplier_return_storno', 'customer_return_storno',
        'popis_visak', 'popis_manjak', 'error_storno', 'error_correction',
        'close_carry'
    )),
    entry_source TEXT NOT NULL DEFAULT 'auto' CHECK (entry_source IN ('auto', 'manual')),
    reference_type TEXT,
    reference_id INTEGER,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kep_entries_book ON kep_entries(book_year, redni_broj);
CREATE INDEX idx_kep_entries_date ON kep_entries(entry_date);
"#,
    },
    Migration {
        version: 13,
        name: "kep_kalkulacije",
        sql: r#"
CREATE TABLE kalkulacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    redni_broj INTEGER NOT NULL,
    book_year INTEGER NOT NULL,
    product_id INTEGER NOT NULL REFERENCES products(id),
    poslovno_ime TEXT NOT NULL,
    prodajno_mesto TEXT NOT NULL,
    pib TEXT NOT NULL,
    trgovacki_naziv TEXT NOT NULL,
    jedinica_mere TEXT NOT NULL,
    kolicina_milli INTEGER NOT NULL,
    nabavna_cena_po_jm_minor INTEGER NOT NULL,
    vrednost_po_fakturi_minor INTEGER NOT NULL,
    razlika_u_ceni_minor INTEGER NOT NULL,
    prodajna_vrednost_bez_pdv_minor INTEGER NOT NULL,
    pdv_minor INTEGER NOT NULL,
    prodajna_vrednost_sa_pdv_minor INTEGER NOT NULL,
    prodajna_cena_po_jm_minor INTEGER NOT NULL,
    reference_type TEXT,
    reference_id INTEGER,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kalkulacije_product ON kalkulacije(product_id, created_at);

ALTER TABLE kep_entries ADD COLUMN cause TEXT;
"#,
    },
    Migration {
        version: 14,
        name: "kep_closures",
        sql: r#"
CREATE TABLE kep_closures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL UNIQUE,
    krajnji_saldo_minor INTEGER NOT NULL,
    entry_count INTEGER NOT NULL,
    closed_at TEXT NOT NULL,
    closed_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_kep_closures_year ON kep_closures(book_year);
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

            // Seed representative installed-base data using only v1 columns so the
            // forward migration can be proven to preserve it (no data loss).
            conn.execute(
                "INSERT INTO users (username, display_name, role, active, created_at, updated_at)
                 VALUES ('stara_kasirka', 'Stara Kasirka', 'cashier', 1, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
                [],
            )
            .expect("v1 users row should insert");
            // backup_jobs exists at v1 and is rebuilt by v2 (CREATE new / copy / DROP /
            // RENAME); a seeded row proves the rebuild actually copies existing data.
            conn.execute(
                "INSERT INTO backup_jobs (backup_type, path, status, error_message, created_at)
                 VALUES ('manual', 'D:/backup/stari.sqlite3', 'completed', NULL, '2025-02-02T08:00:00Z')",
                [],
            )
            .expect("v1 backup_jobs row should insert");

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

            // The pre-existing users row must survive the v3/v4/v5 ALTERs intact.
            let (username, role, active): (String, String, i64) = conn
                .query_row(
                    "SELECT username, role, active FROM users WHERE username = 'stara_kasirka'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("seeded users row should survive forward migration");
            assert_eq!(username, "stara_kasirka");
            assert_eq!(role, "cashier");
            assert_eq!(active, 1);

            // The pre-existing backup_jobs row must survive the v2 table rebuild
            // (CREATE new / copy / DROP / RENAME) with its original values intact.
            let (backup_type, backup_path, status): (String, String, String) = conn
                .query_row(
                    "SELECT backup_type, path, status FROM backup_jobs WHERE path = 'D:/backup/stari.sqlite3'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("seeded backup_jobs row should survive the v2 rebuild");
            assert_eq!(backup_type, "manual");
            assert_eq!(backup_path, "D:/backup/stari.sqlite3");
            assert_eq!(status, "completed");

            // Re-running migrations on an up-to-date database is a no-op.
            run_migrations(&mut conn).expect("re-running migrations should be a no-op");
            let applied_again: i64 = conn
                .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                .expect("migration count should query");
            assert_eq!(applied_again, MIGRATIONS.len() as i64);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// Migration v10 rebuilds price_history to widen the `source` CHECK. The log is
    /// append-only and its ids are evidentiary: a rebuild is a schema migration, not a
    /// data mutation, so every row must land in the new table with its original id.
    /// Seeding non-contiguous ids at v9 is what makes this provable — a rebuild that
    /// dropped `id` from the copy would silently renumber them 1, 2, 3.
    #[test]
    fn migration_v10_rebuild_copies_price_history_rows_with_original_ids() {
        let path = test_database_path("migrations_v10_price_history_ids");

        {
            let mut conn = Connection::open(&path).expect("connection should open");

            // Bring the database to v9 — price_history exists, the campaign rebuild has
            // not run yet — through the same path an installed database takes.
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
            for migration in MIGRATIONS.iter().take_while(|m| m.version <= 9) {
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                    params![migration.version, migration.name],
                )
                .expect("migration should record");
            }

            // Installed-base data: an offered-price log with non-contiguous ids, the
            // shape a real till accumulates once rows are appended over time.
            conn.execute_batch(
                r#"
INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
VALUES (1, 'Opšta stopa', 2000, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z');
INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                      tax_rate_id, minimum_stock_milli, created_at, updated_at)
VALUES (1, 'Šećer 1kg', 'SKU-SECER', 1290000, 0, 1, 0,
        '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z');
INSERT INTO price_history (id, product_id, effective_from, price_minor, source, user_id, created_at)
VALUES (7,  1, '2025-03-01T09:00:00Z', 1290000, 'create', NULL, '2025-03-01T09:00:00Z'),
       (42, 1, '2025-05-10T09:00:00Z', 1190000, 'update', NULL, '2025-05-10T09:00:00Z'),
       (99, 1, '2025-06-20T09:00:00Z', 1090000, 'import', NULL, '2025-06-20T09:00:00Z');
"#,
            )
            .expect("v9 price_history rows should seed");

            // The real installed-base upgrade path: v10 rebuilds price_history.
            run_migrations(&mut conn).expect("forward migration should succeed");

            let mut stmt = conn
                .prepare("SELECT id, product_id, effective_from, price_minor, source FROM price_history ORDER BY id")
                .expect("price_history should prepare");
            let rows: Vec<(i64, i64, String, i64, String)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .expect("price_history should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("price_history rows should collect");

            assert_eq!(
                rows,
                vec![
                    (
                        7,
                        1,
                        "2025-03-01T09:00:00Z".to_string(),
                        1290000,
                        "create".to_string()
                    ),
                    (
                        42,
                        1,
                        "2025-05-10T09:00:00Z".to_string(),
                        1190000,
                        "update".to_string()
                    ),
                    (
                        99,
                        1,
                        "2025-06-20T09:00:00Z".to_string(),
                        1090000,
                        "import".to_string()
                    ),
                ],
                "the v10 rebuild must copy every price_history row verbatim, id included"
            );

            // AUTOINCREMENT's high-water mark must follow the rebuild, or the next
            // appended row would collide with a copied id.
            let sequence: i64 = conn
                .query_row(
                    "SELECT seq FROM sqlite_sequence WHERE name = 'price_history'",
                    [],
                    |row| row.get(0),
                )
                .expect("price_history sequence should survive the rebuild");
            assert_eq!(sequence, 99, "sqlite_sequence must follow the RENAME");

            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-07-17T09:00:00Z', 990000, 'campaign_start', '2026-07-17T09:00:00Z')",
                [],
            )
            .expect("a campaign append should insert after the rebuild");
            let appended_id: i64 = conn
                .query_row(
                    "SELECT id FROM price_history WHERE source = 'campaign_start'",
                    [],
                    |row| row.get(0),
                )
                .expect("appended row should query");
            assert_eq!(
                appended_id, 100,
                "appends must continue past the copied ids, never reuse them"
            );
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
