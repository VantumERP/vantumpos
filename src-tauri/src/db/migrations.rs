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
    Migration {
        version: 15,
        name: "shop_profile_cash_and_declaration_compliance",
        sql: r#"
ALTER TABLE products ADD COLUMN manufacturer_name TEXT;
ALTER TABLE products ADD COLUMN importer_name TEXT;
ALTER TABLE products ADD COLUMN country_of_origin TEXT;
ALTER TABLE products ADD COLUMN official_goods_code TEXT;
ALTER TABLE products ADD COLUMN barcode_kind TEXT
    CHECK (barcode_kind IS NULL OR barcode_kind IN ('gtin', 'internal', 'none'));
ALTER TABLE products ADD COLUMN declaration_checked_at TEXT;
ALTER TABLE products ADD COLUMN declaration_checked_by INTEGER REFERENCES users(id);

ALTER TABLE sales ADD COLUMN aml_cash_minor INTEGER;
ALTER TABLE sales ADD COLUMN aml_rate_minor INTEGER;
ALTER TABLE sales ADD COLUMN aml_rate_date TEXT;
ALTER TABLE sales ADD COLUMN aml_rate_source TEXT;
ALTER TABLE sales ADD COLUMN aml_ack_reason TEXT;

CREATE TABLE cash_movements_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shift_id INTEGER NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    movement_type TEXT NOT NULL CHECK (movement_type IN ('pay_in', 'pay_out', 'bank_deposit', 'bank_withdrawal')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    reason TEXT,
    bank_reference TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
INSERT INTO cash_movements_next (id, shift_id, movement_type, amount_minor, reason, bank_reference, user_id, created_at)
SELECT id, shift_id, movement_type, amount_minor, reason, NULL, user_id, created_at FROM cash_movements;
DROP TABLE cash_movements;
ALTER TABLE cash_movements_next RENAME TO cash_movements;
CREATE INDEX idx_cash_movements_shift ON cash_movements(shift_id);
CREATE INDEX idx_cash_movements_created_at ON cash_movements(created_at);

CREATE TABLE sale_payments_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'card', 'bank_transfer')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor <> 0),
    created_at TEXT NOT NULL
);
INSERT INTO sale_payments_next (id, sale_id, payment_method, amount_minor, created_at)
SELECT id, sale_id, payment_method, amount_minor, created_at FROM sale_payments;
DROP TABLE sale_payments;
ALTER TABLE sale_payments_next RENAME TO sale_payments;
CREATE INDEX idx_sale_payments_sale ON sale_payments(sale_id);

CREATE TABLE non_working_days (
    day TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#,
    },
    Migration {
        version: 16,
        name: "aml_compliance_event_and_documented_cash_movements",
        sql: r#"
-- SQLite cannot alter a CHECK, so widening compliance_log.event_type to admit the
-- AML cash-threshold event is a table rebuild in the style of the v15 rebuilds.
CREATE TABLE compliance_log_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('trading_data_reset', 'backup_restored', 'aml_cash_threshold')),
    detail_json TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
INSERT INTO compliance_log_next (id, event_type, detail_json, user_id, created_at)
SELECT id, event_type, detail_json, user_id, created_at FROM compliance_log;
DROP TABLE compliance_log;
ALTER TABLE compliance_log_next RENAME TO compliance_log;
CREATE INDEX idx_compliance_log_created_at ON compliance_log(created_at);

-- Nullable and deliberately NOT backfilled: NULL means the operator has not
-- asserted that this movement is documented per the pravilnik, which is the safe
-- default for anything that keys off the assertion.
ALTER TABLE cash_movements ADD COLUMN documented_per_pravilnik INTEGER
    CHECK (documented_per_pravilnik IS NULL OR documented_per_pravilnik IN (0, 1));
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
    use crate::db::{test_database_path, Db};

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

    #[test]
    fn migration_v15_adds_declaration_and_cash_compliance_schema() {
        let path = test_database_path("migration_v15_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in [
                "manufacturer_name",
                "importer_name",
                "country_of_origin",
                "official_goods_code",
                "barcode_kind",
                "declaration_checked_at",
                "declaration_checked_by",
            ] {
                assert!(
                    column_exists(&conn, "products", column),
                    "products.{column} should exist after v15"
                );
            }

            for column in [
                "aml_cash_minor",
                "aml_rate_minor",
                "aml_rate_date",
                "aml_rate_source",
                "aml_ack_reason",
            ] {
                assert!(
                    column_exists(&conn, "sales", column),
                    "sales.{column} should exist after v15"
                );
            }

            assert!(column_exists(&conn, "cash_movements", "bank_reference"));

            // The widened CHECKs must actually admit the new values.
            conn.execute_batch(
                "INSERT INTO non_working_days (day, label, created_at)
                 VALUES ('2027-01-07', 'Božić', '2026-07-31T00:00:00Z');",
            )
            .expect("non_working_days should accept a row");
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// v15 is the first rebuild in this schema's history to touch tables that hold
    /// live money: `cash_movements` drives expected cash and the shortfall check, and
    /// `sale_payments` drives Z-reports, dnevni promet and KEP. An installed till is
    /// upgraded in place, so the copy step is the whole point of the rebuild — this
    /// test seeds at v14 and upgrades, which is the only way to prove it.
    #[test]
    fn migration_v15_preserves_pre_existing_cash_movements_and_sale_payments() {
        let path = test_database_path("migration_v15_preserves_money_rows");

        {
            let mut conn = Connection::open(&path).expect("connection should open");

            // Bring the database to v14 — the last schema before the money-table
            // rebuild — through the path a real installed database took.
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
            for migration in &MIGRATIONS[..14] {
                assert!(
                    migration.version <= 14,
                    "the pre-v15 prefix must stop at v14, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                    params![migration.version, migration.name],
                )
                .expect("migration should record");
            }

            // Installed-base money data, seeded with v14 columns only. Ids are explicit
            // and non-contiguous: cash_movements.shift_id and sale_payments.sale_id
            // integrity depends on the rebuild carrying ids across verbatim.
            conn.execute_batch(
                r#"
INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
VALUES (900, 'stari_kasir', 'Stari Kasir', 'cashier', 1,
        '2026-06-01T07:00:00Z', '2026-06-01T07:00:00Z');
INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor,
                    status, created_at, updated_at)
VALUES (900, 900, '2026-06-01T07:00:00Z', 100000, 100000, 'open',
        '2026-06-01T07:00:00Z', '2026-06-01T07:00:00Z');
INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status,
                   subtotal_minor, discount_minor, tax_minor, total_minor,
                   created_at, updated_at)
VALUES (900, 'R-900', 900, 900, 'completed', 100000, 0, 20000, 120000,
        '2026-06-01T09:30:00Z', '2026-06-01T09:30:00Z');

-- Every v14 cash_movements column non-null, so a dropped column is visible too.
INSERT INTO cash_movements (id, shift_id, movement_type, amount_minor, reason,
                            user_id, created_at)
VALUES (57, 900, 'pay_out', 250000, 'Isplata dobavljaču', 900,
        '2026-06-01T11:15:00Z');

-- One positive cash and one NEGATIVE card row: v6 widened the CHECK to signed
-- amounts, so a refund leg is real installed-base data the copy must carry.
INSERT INTO sale_payments (id, sale_id, payment_method, amount_minor, created_at)
VALUES (11, 900, 'cash', 120000, '2026-06-01T09:30:00Z'),
       (77, 900, 'card', -45000, '2026-06-02T10:05:00Z');
"#,
            )
            .expect("v14 money rows should seed");

            // The real installed-base upgrade path: v15 rebuilds both tables.
            run_migrations(&mut conn).expect("forward migration should succeed");

            let movement_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM cash_movements", [], |row| row.get(0))
                .expect("cash_movements count should query");
            assert_eq!(
                movement_count, 1,
                "the v15 rebuild must not lose a single cash movement"
            );

            let (
                id,
                shift_id,
                movement_type,
                amount_minor,
                reason,
                user_id,
                created_at,
                bank_reference,
            ): (i64, i64, String, i64, String, i64, String, Option<String>) = conn
                .query_row(
                    "SELECT id, shift_id, movement_type, amount_minor, reason, user_id,
                            created_at, bank_reference
                     FROM cash_movements",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                        ))
                    },
                )
                .expect("the seeded cash movement must survive the v15 rebuild");
            assert_eq!(id, 57, "the movement id must be carried across verbatim");
            assert_eq!(shift_id, 900, "the shift link must survive the rebuild");
            assert_eq!(movement_type, "pay_out");
            assert_eq!(amount_minor, 250000);
            assert_eq!(reason, "Isplata dobavljaču");
            assert_eq!(user_id, 900);
            assert_eq!(created_at, "2026-06-01T11:15:00Z");
            assert_eq!(
                bank_reference, None,
                "a carried-forward movement predates bank references"
            );

            let payment_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM sale_payments", [], |row| row.get(0))
                .expect("sale_payments count should query");
            assert_eq!(
                payment_count, 2,
                "the v15 rebuild must not lose a single payment leg"
            );

            let mut stmt = conn
                .prepare(
                    "SELECT id, sale_id, payment_method, amount_minor, created_at
                     FROM sale_payments ORDER BY id",
                )
                .expect("sale_payments should prepare");
            let payments: Vec<(i64, i64, String, i64, String)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .expect("sale_payments should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("sale_payments rows should collect");
            assert_eq!(
                payments,
                vec![
                    (
                        11,
                        900,
                        "cash".to_string(),
                        120000,
                        "2026-06-01T09:30:00Z".to_string()
                    ),
                    (
                        77,
                        900,
                        "card".to_string(),
                        -45000,
                        "2026-06-02T10:05:00Z".to_string()
                    ),
                ],
                "the v15 rebuild must copy every payment leg verbatim, signed amount and id included"
            );
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn migration_v15_admits_the_new_movement_types_and_tender() {
        let path = test_database_path("migration_v15_new_values");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            // The rebuild already ran during Db::new; prove a round-trip through
            // the rebuilt tables keeps every column, then prove the new CHECK
            // values are accepted and a bogus one is rejected.
            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (900, 'kasir9', 'Kasir Devet', 'cashier', 1, '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
                     VALUES (900, 900, '2026-07-01T08:00:00Z', 100000, 100000, 'open', '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO cash_movements (shift_id, movement_type, amount_minor, reason, user_id, created_at)
                     VALUES (900, 'pay_in', 5000, 'sitno', 900, '2026-07-01T09:00:00Z');",
            )
            .expect("seed should insert");

            for kind in ["bank_deposit", "bank_withdrawal"] {
                conn.execute(
                    "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, bank_reference, user_id, created_at)
                     VALUES (900, ?1, 2500, 'izvod-1', 900, '2026-07-01T10:00:00Z')",
                    rusqlite::params![kind],
                )
                .unwrap_or_else(|error| panic!("v15 should admit {kind}: {error}"));
            }

            assert!(
                conn.execute(
                    "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, user_id, created_at)
                     VALUES (900, 'teleport', 100, 900, '2026-07-01T11:00:00Z')",
                    [],
                )
                .is_err(),
                "the CHECK must still reject an unknown movement type"
            );

            let kept: i64 = conn
                .query_row(
                    "SELECT amount_minor FROM cash_movements WHERE movement_type = 'pay_in' AND shift_id = 900",
                    [],
                    |row| row.get(0),
                )
                .expect("a pay_in row must still round-trip through the v15 table");
            assert_eq!(kept, 5000);

            // The rebuilt sale_payments must round-trip every column and admit the
            // third tender, while still rejecting an unknown one.
            conn.execute_batch(
                "INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status,
                                    subtotal_minor, discount_minor, tax_minor, total_minor,
                                    created_at, updated_at)
                     VALUES (900, 'R-900', 900, 900, 'completed', 100000, 0, 20000, 120000,
                             '2026-07-01T09:30:00Z', '2026-07-01T09:30:00Z');
                 INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (900, 'cash', 120000, '2026-07-01T09:30:00Z');",
            )
            .expect("sale seed should insert");

            conn.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (900, 'bank_transfer', 4500, '2026-07-01T09:40:00Z')",
                [],
            )
            .expect("v15 should admit bank_transfer");

            assert!(
                conn.execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (900, 'barter', 100, '2026-07-01T09:45:00Z')",
                    [],
                )
                .is_err(),
                "the CHECK must still reject an unknown payment method"
            );

            let cash_kept: i64 = conn
                .query_row(
                    "SELECT amount_minor FROM sale_payments WHERE sale_id = 900 AND payment_method = 'cash'",
                    [],
                    |row| row.get(0),
                )
                .expect("a cash payment must still round-trip through the v15 table");
            assert_eq!(cash_kept, 120000);
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// v16 rebuilds `compliance_log` — the never-deleted audit trail whose rows are
    /// the evidence that a trading-data reset or a backup restore happened at all.
    /// An installed till is upgraded in place, so the copy step is the whole point
    /// of the rebuild; seeding at v15 and upgrading is the only way to prove it.
    /// The same upgrade adds `cash_movements.documented_per_pravilnik`, which must
    /// arrive NULL on every carried-forward movement: NULL means the operator has
    /// not asserted anything, and the exclusion it gates must default OFF.
    #[test]
    fn migration_v16_preserves_pre_existing_compliance_log_and_cash_movements() {
        let path = test_database_path("migration_v16_preserves_audit_rows");

        {
            let mut conn = Connection::open(&path).expect("connection should open");

            // Bring the database to v15 — the last schema before the compliance_log
            // rebuild — through the path a real installed database took.
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
            for migration in &MIGRATIONS[..15] {
                assert!(
                    migration.version <= 15,
                    "the pre-v16 prefix must stop at v15, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                    params![migration.version, migration.name],
                )
                .expect("migration should record");
            }

            // Installed-base audit and money data, seeded with v15 columns only. Ids
            // are explicit and non-contiguous: the audit trail is read in id order and
            // a movement's shift link depends on ids carrying across verbatim.
            conn.execute_batch(
                r#"
INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
VALUES (900, 'stari_kasir', 'Stari Kasir', 'cashier', 1,
        '2026-06-01T07:00:00Z', '2026-06-01T07:00:00Z');
INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor,
                    status, created_at, updated_at)
VALUES (900, 900, '2026-06-01T07:00:00Z', 100000, 100000, 'open',
        '2026-06-01T07:00:00Z', '2026-06-01T07:00:00Z');

-- Both v8 event types, every column non-null, so a dropped column is visible too.
INSERT INTO compliance_log (id, event_type, detail_json, user_id, created_at)
VALUES (13, 'trading_data_reset',
        '{"razlog":"probni podaci obrisani pre početka rada"}', 900,
        '2026-06-01T08:00:00Z'),
       (41, 'backup_restored',
        '{"putanja":"D:/rezerva/kopija.sqlite3"}', 900,
        '2026-06-02T08:30:00Z');

-- Every v15 cash_movements column non-null, bank_reference included.
INSERT INTO cash_movements (id, shift_id, movement_type, amount_minor, reason,
                            bank_reference, user_id, created_at)
VALUES (57, 900, 'bank_deposit', 250000, 'Polog pazara', 'izvod-77', 900,
        '2026-06-01T11:15:00Z');
"#,
            )
            .expect("v15 audit and money rows should seed");

            // The real installed-base upgrade path: v16 rebuilds compliance_log.
            run_migrations(&mut conn).expect("forward migration should succeed");

            let log_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM compliance_log", [], |row| row.get(0))
                .expect("compliance_log count should query");
            assert_eq!(
                log_count, 2,
                "the v16 rebuild must not lose a single audit row"
            );

            let mut stmt = conn
                .prepare(
                    "SELECT id, event_type, detail_json, user_id, created_at
                     FROM compliance_log ORDER BY id",
                )
                .expect("compliance_log should prepare");
            let entries: Vec<(i64, String, String, i64, String)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .expect("compliance_log should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("compliance_log rows should collect");
            assert_eq!(
                entries,
                vec![
                    (
                        13,
                        "trading_data_reset".to_string(),
                        "{\"razlog\":\"probni podaci obrisani pre početka rada\"}".to_string(),
                        900,
                        "2026-06-01T08:00:00Z".to_string()
                    ),
                    (
                        41,
                        "backup_restored".to_string(),
                        "{\"putanja\":\"D:/rezerva/kopija.sqlite3\"}".to_string(),
                        900,
                        "2026-06-02T08:30:00Z".to_string()
                    ),
                ],
                "the v16 rebuild must copy every audit row verbatim, id and detail included"
            );

            // The rebuild drops the table, so the index it was read through must be
            // back — the audit trail is queried by created_at.
            let index_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index' AND name = 'idx_compliance_log_created_at'",
                    [],
                    |row| row.get(0),
                )
                .expect("index metadata should query");
            assert_eq!(
                index_count, 1,
                "the v16 rebuild must recreate idx_compliance_log_created_at"
            );

            // cash_movements is only ALTERed, never rebuilt — the row must be intact
            // and the new flag must arrive NULL, never backfilled to an assertion.
            let movement_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM cash_movements", [], |row| row.get(0))
                .expect("cash_movements count should query");
            assert_eq!(
                movement_count, 1,
                "v16 must not disturb a single cash movement"
            );

            let (
                id,
                shift_id,
                movement_type,
                amount_minor,
                reason,
                bank_reference,
                user_id,
                created_at,
                documented,
            ): (
                i64,
                i64,
                String,
                i64,
                String,
                String,
                i64,
                String,
                Option<i64>,
            ) = conn
                .query_row(
                    "SELECT id, shift_id, movement_type, amount_minor, reason,
                            bank_reference, user_id, created_at, documented_per_pravilnik
                     FROM cash_movements",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                            row.get(8)?,
                        ))
                    },
                )
                .expect("the seeded cash movement must survive v16");
            assert_eq!(id, 57, "the movement id must be carried across verbatim");
            assert_eq!(shift_id, 900, "the shift link must survive v16");
            assert_eq!(movement_type, "bank_deposit");
            assert_eq!(amount_minor, 250000);
            assert_eq!(reason, "Polog pazara");
            assert_eq!(bank_reference, "izvod-77");
            assert_eq!(user_id, 900);
            assert_eq!(created_at, "2026-06-01T11:15:00Z");
            assert_eq!(
                documented, None,
                "an upgraded movement carries no operator assertion, so the flag must be NULL"
            );
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn migration_v16_admits_the_aml_event_and_the_documented_flag() {
        let path = test_database_path("migration_v16_new_values");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (900, 'kasir9', 'Kasir Devet', 'cashier', 1, '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
                     VALUES (900, 900, '2026-07-01T08:00:00Z', 100000, 100000, 'open', '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');",
            )
            .expect("seed should insert");

            // The widened CHECK must admit the AML event alongside the v8 pair.
            for event_type in [
                "trading_data_reset",
                "backup_restored",
                "aml_cash_threshold",
            ] {
                conn.execute(
                    "INSERT INTO compliance_log (event_type, detail_json, user_id, created_at)
                     VALUES (?1, '{\"napomena\":\"provera praga\"}', 900, '2026-07-01T09:00:00Z')",
                    rusqlite::params![event_type],
                )
                .unwrap_or_else(|error| panic!("v16 should admit {event_type}: {error}"));
            }

            assert!(
                conn.execute(
                    "INSERT INTO compliance_log (event_type, user_id, created_at)
                     VALUES ('teleport', 900, '2026-07-01T09:05:00Z')",
                    [],
                )
                .is_err(),
                "the CHECK must still reject an unknown compliance event type"
            );

            let aml_kept: String = conn
                .query_row(
                    "SELECT detail_json FROM compliance_log WHERE event_type = 'aml_cash_threshold'",
                    [],
                    |row| row.get(0),
                )
                .expect("an AML event must round-trip through the v16 table");
            assert_eq!(aml_kept, "{\"napomena\":\"provera praga\"}");

            // The new flag is tri-state: NULL (no assertion), 0 and 1.
            for documented in [None, Some(0_i64), Some(1_i64)] {
                conn.execute(
                    "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, reason,
                                                 documented_per_pravilnik, user_id, created_at)
                     VALUES (900, 'pay_out', 5000, 'sitno', ?1, 900, '2026-07-01T10:00:00Z')",
                    rusqlite::params![documented],
                )
                .unwrap_or_else(|error| {
                    panic!("v16 should admit documented_per_pravilnik = {documented:?}: {error}")
                });
            }

            assert!(
                conn.execute(
                    "INSERT INTO cash_movements (shift_id, movement_type, amount_minor,
                                                 documented_per_pravilnik, user_id, created_at)
                     VALUES (900, 'pay_out', 5000, 2, 900, '2026-07-01T10:30:00Z')",
                    [],
                )
                .is_err(),
                "the CHECK must reject a documented_per_pravilnik outside NULL/0/1"
            );

            let unasserted: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM cash_movements WHERE documented_per_pravilnik IS NULL",
                    [],
                    |row| row.get(0),
                )
                .expect("documented_per_pravilnik count should query");
            assert_eq!(
                unasserted, 1,
                "a movement written without the flag must stay unasserted, not default to 1"
            );
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
