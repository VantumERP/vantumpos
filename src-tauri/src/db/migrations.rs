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
    Migration {
        version: 17,
        name: "worktime_records_and_retention",
        sql: r#"
-- Every ISO date column carries a GLOB shape check. '2026-8-3' and '2026-08-03'
-- are the same calendar day to a human and two different keys to SQLite, which
-- would open a second slot per (zaposleni, dan) and drop those minutes out of the
-- čl. 53 weekly bucket.
ALTER TABLE users ADD COLUMN datum_rodjenja TEXT CHECK (datum_rodjenja IS NULL OR datum_rodjenja GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN datum_rodjenja_najmladjeg_deteta TEXT CHECK (datum_rodjenja_najmladjeg_deteta IS NULL OR datum_rodjenja_najmladjeg_deteta GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN samohrani_roditelj INTEGER CHECK (samohrani_roditelj IS NULL OR samohrani_roditelj IN (0, 1));
-- ZoR čl. 91 st. 2 has TWO legs: „dete do sedam godina života“ ILI „dete koje je
-- težak invalid“. The second leg carries no age limit, so it needs its own flag —
-- without it a samohrani roditelj of a disabled child aged 8+ gets no consent gate.
ALTER TABLE users ADD COLUMN dete_tezak_invalid INTEGER CHECK (dete_tezak_invalid IS NULL OR dete_tezak_invalid IN (0, 1));
ALTER TABLE users ADD COLUMN trudnoca_ili_dojenje INTEGER CHECK (trudnoca_ili_dojenje IS NULL OR trudnoca_ili_dojenje IN (0, 1));
ALTER TABLE users ADD COLUMN trudnoca_ili_dojenje_od TEXT CHECK (trudnoca_ili_dojenje_od IS NULL OR trudnoca_ili_dojenje_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN radi_u_preraspodeli INTEGER NOT NULL DEFAULT 0 CHECK (radi_u_preraspodeli IN (0, 1));
ALTER TABLE users ADD COLUMN ugovoreno_radno_vreme_minuta_nedeljno INTEGER;
ALTER TABLE users ADD COLUMN zanimanje_sifra TEXT;
ALTER TABLE users ADD COLUMN kvalifikacija_sifra TEXT;
-- Two legally distinct written consents. They are NOT interchangeable and must
-- never be read for each other's purpose:
--   saglasnost_prekovremeni_od — ZoR čl. 91: a protected parent consenting to
--     prekovremeni/noćni rad.
--   saglasnost_preraspodela_od — ZoR čl. 57 st. 4: a zaposleni „koji se saglasio“
--     to average longer in preraspodela, so hours above the average are computed
--     and paid as prekovremeni rad.
-- Neither is a ZZPL pristanak. The column records that a written consent exists
-- and from when; it does not collect one, and no consent UI belongs anywhere in
-- the employee surface.
ALTER TABLE users ADD COLUMN saglasnost_prekovremeni_od TEXT CHECK (saglasnost_prekovremeni_od IS NULL OR saglasnost_prekovremeni_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
ALTER TABLE users ADD COLUMN saglasnost_preraspodela_od TEXT CHECK (saglasnost_preraspodela_od IS NULL OR saglasnost_preraspodela_od GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');

CREATE TABLE work_time_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    dan TEXT NOT NULL CHECK (dan GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    -- The correction chain is strictly linear: verzija 1 is the original and every
    -- ispravka appends verzija + 1. The live row for a day is MAX(verzija), which
    -- makes „the live row“ a well-defined query without a single UPDATE anywhere.
    verzija INTEGER NOT NULL DEFAULT 1 CHECK (verzija >= 1),
    moguci_minuta INTEGER NOT NULL DEFAULT 0 CHECK (moguci_minuta >= 0),
    ukupno_ostvareni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (ukupno_ostvareni_minuta >= 0),
    efektivno_izvrseni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (efektivno_izvrseni_minuta >= 0),
    casovi_cekanja_i_zastoja_minuta INTEGER NOT NULL DEFAULT 0 CHECK (casovi_cekanja_i_zastoja_minuta >= 0),
    obustava_rada_strajk_minuta INTEGER NOT NULL DEFAULT 0 CHECK (obustava_rada_strajk_minuta >= 0),
    ukupno_neizvrseni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (ukupno_neizvrseni_minuta >= 0),
    godisnji_odmor_minuta INTEGER NOT NULL DEFAULT 0 CHECK (godisnji_odmor_minuta >= 0),
    praznik_odmor_minuta INTEGER NOT NULL DEFAULT 0 CHECK (praznik_odmor_minuta >= 0),
    odsustvo_uz_naknadu_minuta INTEGER NOT NULL DEFAULT 0 CHECK (odsustvo_uz_naknadu_minuta >= 0),
    strucno_osposobljavanje_minuta INTEGER NOT NULL DEFAULT 0 CHECK (strucno_osposobljavanje_minuta >= 0),
    sprecenost_poslodavac_minuta INTEGER NOT NULL DEFAULT 0 CHECK (sprecenost_poslodavac_minuta >= 0),
    naknada_drugi_poslodavci_minuta INTEGER NOT NULL DEFAULT 0 CHECK (naknada_drugi_poslodavci_minuta >= 0),
    sprecenost_rfzo_minuta INTEGER NOT NULL DEFAULT 0 CHECK (sprecenost_rfzo_minuta >= 0),
    porodiljsko_minuta INTEGER NOT NULL DEFAULT 0 CHECK (porodiljsko_minuta >= 0),
    neplaceno_odsustvo_minuta INTEGER NOT NULL DEFAULT 0 CHECK (neplaceno_odsustvo_minuta >= 0),
    prekovremeni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (prekovremeni_minuta >= 0),
    nocni_minuta INTEGER NOT NULL DEFAULT 0 CHECK (nocni_minuta >= 0),
    rad_na_praznik_minuta INTEGER NOT NULL DEFAULT 0 CHECK (rad_na_praznik_minuta >= 0),
    -- Closed enum, and every value is exactly its bucket column minus the `_minuta`
    -- suffix: godisnji_odmor books into godisnji_odmor_minuta, obustava_rada_strajk
    -- into obustava_rada_strajk_minuta. Category → bucket is therefore DERIVED in
    -- code, never a hand-maintained lookup table where one wrong line would post
    -- ZEOR čl. 24 tač. 1 d) hours into the g) bucket with nothing downstream able to
    -- notice. Both vocabularies keep the statutory wording of čl. 24 tač. 1, so the
    -- rule costs no legal fidelity — it only forbids the two from drifting apart.
    kategorija_odsustva TEXT CHECK (kategorija_odsustva IS NULL OR kategorija_odsustva IN (
        'godisnji_odmor', 'praznik_odmor', 'odsustvo_uz_naknadu', 'strucno_osposobljavanje',
        'sprecenost_poslodavac', 'sprecenost_rfzo', 'porodiljsko', 'neplaceno_odsustvo',
        'naknada_drugi_poslodavci', 'obustava_rada_strajk'
    )),
    -- NOT A LEGAL JUSTIFICATION. These are the ZoR čl. 53 st. 1 grounds on which
    -- overtime may be ORDERED; recording one does not make a čl. 53 st. 2/3 cap
    -- breach lawful. Closed enum, because no unconstrained TEXT may exist on this
    -- table — see korekcija_razlog below.
    cap_override_razlog TEXT CHECK (cap_override_razlog IS NULL OR cap_override_razlog IN (
        'visa_sila', 'iznenadno_povecanje_obima_posla', 'neplanirani_posao_u_roku', 'drugo'
    )),
    supersedes_id INTEGER REFERENCES work_time_entries(id),
    -- ZERO free text on an absence row (§4 req. 3, §5 item 4). A free-text column
    -- on the row that also carries kategorija_odsustva and the two sprečenost
    -- buckets would eventually hold a diagnosis, an ICD code or a doznaka number,
    -- on a shop-counter PC reachable over remote support. Closed enum, no escape.
    korekcija_razlog TEXT CHECK (korekcija_razlog IS NULL OR korekcija_razlog IN (
        'greska_u_unosu', 'ispravka_sati', 'ispravka_kategorije',
        'naknadno_dostavljen_dokument', 'drugo'
    )),
    unio_user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    -- A čl. 53 cap override is only meaningful on a worked day.
    CHECK (kategorija_odsustva IS NULL OR cap_override_razlog IS NULL),
    -- Chain root vs correction can never be confused.
    CHECK ((supersedes_id IS NULL AND verzija = 1) OR (supersedes_id IS NOT NULL AND verzija > 1)),
    -- ZEOR čl. 46 st. 1 puts accuracy responsibility on the record-keeper: a
    -- correction to an append-only statutory record must carry who and why, and the
    -- schema enforces it so no other write path can route around the command layer.
    CHECK (supersedes_id IS NULL OR (unio_user_id IS NOT NULL AND korekcija_razlog IS NOT NULL))
);
-- Total, not partial. A partial index `WHERE supersedes_id IS NULL` would index
-- only chain ROOTS and let two forked corrections both stay live for one day —
-- one reporting a čl. 53 st. 2 breach and one not.
CREATE UNIQUE INDEX idx_work_time_entries_user_day
    ON work_time_entries(user_id, dan, verzija);
CREATE INDEX idx_work_time_entries_dan ON work_time_entries(dan);
-- SQLite CHECK cannot subquery, so the „same employee, same day, next verzija“
-- leg of the chain invariant is a trigger. Append-only: BEFORE INSERT only, no
-- UPDATE and no DELETE anywhere in this schema.
CREATE TRIGGER trg_work_time_entries_ispravka_isti_dan
BEFORE INSERT ON work_time_entries
WHEN NEW.supersedes_id IS NOT NULL
     AND NOT EXISTS (
         SELECT 1 FROM work_time_entries
          WHERE id = NEW.supersedes_id
            AND user_id = NEW.user_id
            AND dan = NEW.dan
            AND verzija = NEW.verzija - 1
     )
BEGIN
    SELECT RAISE(ABORT, 'Ispravka mora da pripada istom zaposlenom i istom danu i da nastavlja prethodnu verziju.');
END;

CREATE TABLE work_time_periods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    godina INTEGER NOT NULL,
    mesec INTEGER NOT NULL CHECK (mesec BETWEEN 1 AND 12),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    closed_at TEXT,
    closed_by INTEGER REFERENCES users(id),
    klasifikacija_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_work_time_periods_user_month ON work_time_periods(user_id, godina, mesec);

CREATE TABLE retention_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    record_class TEXT NOT NULL UNIQUE,
    retain_until TEXT,
    legal_hold INTEGER NOT NULL DEFAULT 0 CHECK (legal_hold IN (0, 1)),
    never_purge INTEGER NOT NULL DEFAULT 0 CHECK (never_purge IN (0, 1)),
    napomena TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#,
    },
    Migration {
        version: 18,
        name: "zzpl_audit_support_personnel_breach_and_register",
        sql: r#"
-- SW-10, the „nalog“ half. ZZPL čl. 46 is the penalised rule here (čl. 95 st. 1
-- t. 23): an obrađivač — and „drugo lice … ovlašćeno za pristup“, which reaches
-- the individual support engineer — may not process without the rukovalac's
-- nalog. THIS ROW IS THAT NALOG, and evidentially it is worth more than the
-- session log beside it. Čl. 50, by contrast, prescribes no prekršaj at all.
CREATE TABLE support_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    granted_by INTEGER NOT NULL REFERENCES users(id),
    granted_at TEXT NOT NULL,
    -- Free text on purpose, and the only free-text column v18 adds outside the
    -- breach record. A nalog says what the vlasnik authorised, in the vlasnik's
    -- own words; a closed enum would either be too coarse to be a nalog or would
    -- have to be reopened by a migration on every new support scenario. The
    -- audit-log exclusions (req. 4) govern `audit_events`, not this table.
    scope TEXT NOT NULL CHECK (scope <> ''),
    -- „Explicit scope AND duration“ (req. 2). An open-ended grant is not a nalog,
    -- so the expiry is NOT NULL and must sit after the grant.
    expires_at TEXT NOT NULL CHECK (expires_at > granted_at),
    started_at TEXT,
    ended_at TEXT,
    revoked_at TEXT,
    revoked_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (ended_at IS NULL OR started_at IS NOT NULL)
);
CREATE INDEX idx_support_sessions_granted_at ON support_sessions(granted_at);

-- SW-10, the log half — and it is a PRUDENTIAL control, never a legal duty. No
-- ZZPL provision obliges a private rukovalac to record who accessed personal
-- data: čl. 48 and čl. 51 bind only a nadležni organ u posebne svrhe, and čl. 50
-- appears nowhere in čl. 95, so no prekršaj attaches to not having this table.
-- It exists to discharge an OUTCOME duty — čl. 5 st. 2 odgovornost za postupanje
-- and the čl. 41 st. 1 ability to predočiti — and its field list is modelled on
-- čl. 48 st. 2 + čl. 51 st. 2 t. 7, the only two content specs Serbian law has.
CREATE TABLE audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at TEXT NOT NULL,
    -- Čl. 48 st. 2 „identiteta lica“ — an internal user id, NEVER a name column.
    -- Nullable because the čl. 5 st. 1 t. 5 purge is time-driven, not
    -- request-driven (req. 23), so its line has no human actor; a fabricated
    -- actor id would be worse evidence than an honestly absent one.
    actor_user_id INTEGER REFERENCES users(id),
    -- Čl. 48 st. 1 verbatim, transliterated: unos, menjanje, uvid, otkrivanje
    -- (uključujući i prenos), upoređivanje, brisanje. Closed, because a log whose
    -- vocabulary drifts cannot be read back as evidence of anything.
    action TEXT NOT NULL CHECK (action IN (
        'unos', 'menjanje', 'uvid', 'otkrivanje', 'uporedjivanje', 'brisanje'
    )),
    -- A code constant ('sale', 'employee'), so it carries the same no-space shape
    -- as object_id below. Without it, this column is the one free-text hole on the
    -- table and 'pretraga:Marko Marković' fits straight into it.
    object_type TEXT NOT NULL CHECK (object_type <> '' AND object_type NOT GLOB '* *'),
    -- An opaque internal id, never the object's contents (req. 4). The no-space
    -- shape is the schema's share of that rule: names, addresses and search
    -- queries carry spaces, internal ids do not. The full exclusion list — JMBG,
    -- PAN, phone, address, free-text notes, query strings — is enforced at the
    -- write boundary in `audit.rs`, because SQLite cannot run Luhn.
    object_id TEXT NOT NULL CHECK (object_id <> '' AND object_id NOT GLOB '* *'),
    -- Čl. 48 st. 2 requires the RAZLOG for uvid and otkrivanje. Closed enum: the
    -- one column that could plausibly have been free text is the one an operator
    -- would eventually type a customer's name into.
    reason_code TEXT CHECK (reason_code IS NULL OR reason_code IN (
        'inspekcija', 'zahtev_lica', 'interni_nadzor', 'obrada_reklamacije',
        'obracun_zarade', 'tehnicka_podrska', 'sudski_ili_upravni_postupak',
        'bezbednosni_incident', 'zakonska_obaveza', 'automatsko_ciscenje'
    )),
    -- Čl. 48 st. 2 „identiteta primaoca“, recorded as a CLASS of recipient. A
    -- named recipient would be personal data about that recipient, sitting in the
    -- table whose whole point (req. 4) is to hold none — the class answers the
    -- statutory question without reproducing the problem.
    recipient TEXT CHECK (recipient IS NULL OR recipient IN (
        'lice_na_koje_se_podaci_odnose', 'poreska_uprava', 'inspekcija', 'poverenik',
        'sud_ili_javni_tuzilac', 'mup', 'knjigovodja', 'obradjivac_tehnicke_podrske',
        'banka', 'drugi_organ'
    )),
    support_session_id INTEGER REFERENCES support_sessions(id),
    -- Tamper-evidence outranks completeness (req. 7). prev_hash is '' on the
    -- genesis row and the previous row's hash thereafter.
    prev_hash TEXT NOT NULL,
    hash TEXT NOT NULL CHECK (hash <> ''),
    -- The čl. 48 st. 2 razlog is a constraint, not a convention.
    CHECK (action NOT IN ('uvid', 'otkrivanje') OR reason_code IS NOT NULL),
    -- Otkrivanje without a primalac does not answer the question čl. 48 st. 2 asks.
    CHECK (action <> 'otkrivanje' OR recipient IS NOT NULL)
);
CREATE INDEX idx_audit_events_at ON audit_events(at);
CREATE INDEX idx_audit_events_actor ON audit_events(actor_user_id, at);
-- Deliberately BEFORE UPDATE only. DELETE stays open because req. 6 forbids
-- „trajno“ on this log — čl. 47 st. 7 governs the register of processing
-- activities, and copying it here would put the product in permanent breach of
-- storage limitation — so expiry must be able to remove a row. A removed row is
-- not silent: it breaks the hash chain, which is exactly what the chain is for.
CREATE TRIGGER trg_audit_events_bez_izmene
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'Zapis u evidenciji pristupa ne može da se menja.');
END;

-- SW-13 class A — the 25 tačke of ZEOR čl. 5, physically separate from the
-- account store so that the purge job is STRUCTURALLY incapable of reaching them
-- (req. 19). Class B (ledger attribution) is the surrogate `users.id` already on
-- the transaction rows, so no transaction row changes here — and no ime,
-- prezime or matični broj is ever denormalised onto one (req. 20).
CREATE TABLE personnel_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- No ON DELETE clause, so SQLite's default NO ACTION restricts: deleting the
    -- account cannot take the personnel record with it (req. 24). UNIQUE keeps
    -- one record per account; `users.username` has been UNIQUE since v1 and that
    -- uniqueness survives deactivation, which is the deactivate-never-reuse leg.
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id),
    -- ZEOR čl. 5 t. 1–25. Only prezime i ime is required, because a record is
    -- opened on the day work starts (čl. 7 st. 1) and filled in as the data
    -- arrives; „datum i mesto rođenja“ is one tačka over two columns.
    prezime_ime TEXT NOT NULL CHECK (prezime_ime <> ''),
    maticni_broj TEXT,
    pol TEXT CHECK (pol IS NULL OR pol IN ('muski', 'zenski')),
    datum_rodjenja TEXT CHECK (datum_rodjenja IS NULL OR datum_rodjenja GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    mesto_rodjenja TEXT,
    prebivaliste_i_adresa_stana TEXT,
    mesto_rada TEXT,
    naziv_i_adresa_poslodavca TEXT,
    delatnost_poslodavca TEXT,
    zanimanje TEXT,
    vrsta_i_stepen_strucne_spreme TEXT,
    osposobljenost TEXT,
    naziv_radnog_mesta TEXT,
    -- ZEOR says „radno vreme u časovima“; the store is minutes, like every other
    -- duration in this database, and the hours are a render-time division.
    radno_vreme_minuta_nedeljno INTEGER CHECK (radno_vreme_minuta_nedeljno IS NULL OR radno_vreme_minuta_nedeljno >= 0),
    trajanje_zaposlenja TEXT CHECK (trajanje_zaposlenja IS NULL OR trajanje_zaposlenja IN ('neodredjeno', 'odredjeno')),
    vrsta_radnog_odnosa TEXT,
    osnov_upucivanja_u_inostranstvo TEXT,
    naziv_poslodavca_u_dopunskom_radu TEXT,
    zainteresovanost_za_promenu_posla INTEGER CHECK (zainteresovanost_za_promenu_posla IS NULL OR zainteresovanost_za_promenu_posla IN (0, 1)),
    invalid_rada INTEGER CHECK (invalid_rada IS NULL OR invalid_rada IN (0, 1)),
    -- A COUNT, never a roster. The insured family members are third parties whose
    -- data the shop has no purpose to hold on a till-adjacent machine.
    osigurani_clanovi_porodice INTEGER CHECK (osigurani_clanovi_porodice IS NULL OR osigurani_clanovi_porodice >= 0),
    -- Days, never a diagnosis, an ICD code or a doznaka number: health data is a
    -- posebna vrsta podataka under ZZPL čl. 17 and has no home in this table.
    privremena_nesposobnost_dana INTEGER CHECK (privremena_nesposobnost_dana IS NULL OR privremena_nesposobnost_dana >= 0),
    placeno_odsustvo_dana INTEGER CHECK (placeno_odsustvo_dana IS NULL OR placeno_odsustvo_dana >= 0),
    datum_zasnivanja TEXT CHECK (datum_zasnivanja IS NULL OR datum_zasnivanja GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    datum_prestanka TEXT CHECK (datum_prestanka IS NULL OR datum_prestanka GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    razlog_prestanka TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
-- ZEOR čl. 7 st. 2: „Podaci iz evidencije o zaposlenim licima čuvaju se trajno.“
-- The record is opened on the first day of work and closed on the last (st. 1);
-- it is never deleted. A trigger rather than a convention, because the point of
-- req. 19 is that the purge job CANNOT reach class A, not that it currently does
-- not. Corrections are UPDATEs, which stay open — unlike the audit log, this is
-- a living record whose accuracy is itself a duty.
CREATE TRIGGER trg_personnel_records_trajno
BEFORE DELETE ON personnel_records
BEGIN
    SELECT RAISE(ABORT, 'Evidencija o zaposlenim licima čuva se trajno i ne može da se briše.');
END;

-- SW-17. Čl. 52 st. 6 covers „svaku povredu“, so the row always exists and
-- notifiability is a derived flag on it — never a wizard gate that discards the
-- non-notifiable incident (req. 43). St. 7 makes this documentation the vehicle
-- for proving čl. 52 compliance as a whole, which is why the field set goes
-- beyond st. 6's three elements (req. 44).
CREATE TABLE data_breaches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The 72 h anchor. Čl. 52 st. 1 says „od saznanja za povredu“, not from the
    -- incident, so this is NOT NULL and immutable (trigger below).
    saznanje_at TEXT NOT NULL,
    occurred_at TEXT,
    discovered_at TEXT,
    -- §6 R-3: when an obrađivač is in the chain, čl. 52 st. 3 gives two candidate
    -- anchors. Record BOTH and let the operator see which one saznanje_at follows;
    -- silently picking one would be a legal decision made by a schema.
    obradjivac_saznanje_at TEXT,
    rukovalac_obavesten_at TEXT,
    -- The three čl. 52 st. 6 elements.
    opis TEXT NOT NULL,
    posledice TEXT NOT NULL,
    mere TEXT NOT NULL,
    -- Obrazac section 2 point (3): „broj lica na koja se podaci odnose“ — a
    -- COUNT. The prescribed form asks for a number and never for a roster.
    broj_lica INTEGER CHECK (broj_lica IS NULL OR broj_lica >= 0),
    kategorije_podataka TEXT,
    risk_outcome TEXT CHECK (risk_outcome IS NULL OR risk_outcome IN (
        'bez_rizika', 'rizik', 'visok_rizik'
    )),
    notify_decision TEXT CHECK (notify_decision IS NULL OR notify_decision IN (
        'obavestiti', 'ne_obavestiti'
    )),
    notify_obrazlozenje TEXT,
    poverenik_notified_at TEXT,
    -- Čl. 52 st. 2: mandatory once 72 h have passed since saznanje. The deadline
    -- is a decision over a `now: &str`, so it is enforced in the command, not by
    -- a CHECK that would have to call datetime('now') to know the answer.
    delay_reason TEXT,
    -- The separate čl. 53 block: were the affected individuals told, and if not,
    -- which st. 3 exception was relied on.
    lica_obavestena INTEGER CHECK (lica_obavestena IS NULL OR lica_obavestena IN (0, 1)),
    lica_obavestena_at TEXT,
    cl53_izuzetak TEXT CHECK (cl53_izuzetak IS NULL OR cl53_izuzetak IN (
        'primenjene_mere_zastite', 'naknadne_mere', 'nesrazmeran_utrosak_vremena_i_sredstava'
    )),
    cl53_izuzetak_obrazlozenje TEXT,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_data_breaches_saznanje_at ON data_breaches(saznanje_at);
-- Everything else on the record may be corrected as the investigation proceeds;
-- the clock anchor may not. A movable saznanje_at would make the čl. 52 st. 2
-- delay reason optional in hindsight.
CREATE TRIGGER trg_data_breaches_saznanje_nepromenljiv
BEFORE UPDATE OF saznanje_at ON data_breaches
WHEN NEW.saznanje_at <> OLD.saznanje_at
BEGIN
    SELECT RAISE(ABORT, 'Vreme saznanja za povredu je nepromenljivo (ZZPL čl. 52 st. 1).');
END;

-- The čl. 47 evidencija radnji obrade (req. 28) — generated, not hand-kept. This
-- is the cheapest and most likely inspection finding for a three-employee shop
-- and the one issuable on the spot by prekršajni nalog, and Pravilnik 40/2019
-- čl. 4 st. 1 makes it a mandatory attachment to a breach notification.
CREATE TABLE processing_activities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Stable key of the generated activity, so regeneration is an upsert and not
    -- a second copy of the same radnja.
    kljuc TEXT NOT NULL UNIQUE CHECK (kljuc <> ''),
    rukovalac_naziv TEXT NOT NULL,          -- st. 1 t. 1
    rukovalac_kontakt TEXT,                 -- st. 1 t. 1
    svrha_obrade TEXT NOT NULL,             -- st. 1 t. 2
    vrsta_lica TEXT NOT NULL,               -- st. 1 t. 3
    vrsta_podataka TEXT NOT NULL,           -- st. 1 t. 3
    vrsta_primalaca TEXT,                   -- st. 1 t. 4
    prenos_u_druge_drzave TEXT,             -- st. 1 t. 5
    mere_zastite_prenosa TEXT,              -- st. 1 t. 5
    -- St. 1 t. 6, per category: „rok posle čijeg isteka se brišu određene vrste
    -- podataka o ličnosti, AKO JE TAKAV ROK ODREĐEN“ — hence nullable.
    rok_cuvanja TEXT,
    -- and the same rok as a machine-readable link into the shared retention table
    -- SW-14 created, so the register cannot claim a period the app does not apply.
    retention_record_class TEXT REFERENCES retention_policies(record_class),
    opis_mera_zastite TEXT,                 -- st. 1 t. 7 (mere iz čl. 50 st. 1)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#,
    },
    Migration {
        version: 19,
        name: "cenovnik_snapshots_and_unit_price_fields",
        sql: r#"
-- SW-12, the archive half (req. 14). ZZP čl. 6 st. 5 obliges a trader who
-- publishes a cenovnik to enable comparison of „prethodno objavljenih cena“ with
-- „cena objavljenih u realnom vremenu“, so a publication may never be overwritten
-- by the next one — every published file is its own row, kept verbatim.
--
-- There is deliberately NO `updated_at` and NO current-snapshot pointer column.
-- „The current cenovnik for a prodajni objekat“ is DERIVED as that outlet's newest
-- row — MAX(generated_at), ties broken by the larger id — because a stored pointer
-- is a second source of truth that two writers can leave aimed at a snapshot which
-- is no longer the newest, and čl. 6 st. 4 binds the shop to whatever the current
-- one says.
CREATE TABLE cenovnik_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Čl. 6 st. 2 „posebno za svaki prodajni objekat“: one cenovnik per outlet, so
    -- the outlet is part of the archive's identity and a blank one is a snapshot
    -- of nothing.
    prodajno_mesto TEXT NOT NULL CHECK (prodajno_mesto <> ''),
    -- RFC3339, passed in as `now: &str` by the command layer like every other
    -- decision timestamp in this schema. Never datetime('now'). Non-empty: an
    -- undated row sorts below every RFC3339 stamp, so it could never be selected
    -- as an outlet's current cenovnik and could never take part in the st. 5
    -- comparison — a snapshot the archive cannot date is not an archive entry.
    generated_at TEXT NOT NULL CHECK (generated_at <> ''),
    row_count INTEGER NOT NULL CHECK (row_count >= 0),
    content_hash TEXT NOT NULL CHECK (content_hash <> ''),
    -- The rendered file, byte for byte. An archive holding only a hash could not
    -- answer WHAT was published, which is the one question st. 5 exists to allow.
    body TEXT NOT NULL,
    -- NULL until a publish target accepted the body. A generated-but-unpublished
    -- snapshot is an honest and expected state: with no target configured (req. 15
    -- — a founder decision, not an engineering one) generation still happens and a
    -- price save must not fail because of it.
    published_at TEXT,
    published_target TEXT,
    created_at TEXT NOT NULL CHECK (created_at <> '')
);
CREATE INDEX idx_cenovnik_snapshots_prodajno_mesto
    ON cenovnik_snapshots(prodajno_mesto, generated_at);
-- What was published is evidence — čl. 6 st. 4 makes the shop answerable for the
-- prices in it — so NO UPDATE may rewrite a snapshot's body, its hash or its
-- identity. `id` is in the list with the rest: it is the tie-break that decides
-- which row is an outlet's current cenovnik and the only handle a divergence
-- record can name, so a mutable primary key would let that evidence link be
-- repointed at a different published file.
--
-- That is the whole of what the engine enforces, and no more. DELETE is open by
-- design (see čl. 213 below), and so is `INSERT OR REPLACE`: SQLite runs REPLACE
-- as a DELETE plus an INSERT, no UPDATE happens for either trigger to see, and
-- with recursive_triggers off — as this app runs it — the delete half fires no
-- trigger either. Closing that here would close the retention purge with it, so
-- it is a constraint on the write path rather than a claim made here: the publish
-- path uses a plain INSERT only, and the retention purge is the only code
-- permitted to DELETE from this table. Both halves are asserted in the tests.
CREATE TRIGGER trg_cenovnik_snapshots_telo_nepromenljivo
BEFORE UPDATE OF id, prodajno_mesto, generated_at, row_count, content_hash, body, created_at
    ON cenovnik_snapshots
BEGIN
    SELECT RAISE(ABORT, 'Objavljeni cenovnik je nepromenljiv — nova cena je novi snimak.');
END;
-- The one field written after the fact: a snapshot is generated first and only
-- then accepted by a target. Recorded once, because restamping it would move the
-- publication date of a file that was published on a different day.
CREATE TRIGGER trg_cenovnik_snapshots_objava_jednom
BEFORE UPDATE OF published_at, published_target ON cenovnik_snapshots
WHEN OLD.published_at IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'Objava snimka cenovnika je već zabeležena i ne može da se menja.');
END;
-- No DELETE trigger on purpose. Čl. 213 gives the archive a two-year limitation
-- floor, not a „trajno“ duty, so the retention purge must be able to reach an
-- expired snapshot.

-- Req. 10. Čl. 6 st. 2's second sentence pulls st. 1 into the published file, so a
-- cenovnik carrying only the prodajna cena does not discharge the duty.
-- `products.unit_of_measure` (v1) is the measure the goods are SOLD in; this is
-- the measure the jedinična cena is EXPRESSED in, and for a 0,75 l bottle sold by
-- the piece the two differ. Nullable, because a product priced per piece may
-- legitimately have neither.
ALTER TABLE products ADD COLUMN jedinicna_cena_jedinica TEXT;
-- The content of one selling unit in that measure, on the schema-wide milli scale
-- — value × 1000, the same one `quantity_milli` and `minimum_stock_milli` carry —
-- so a 0,75 l bottle is 750. NULL means one selling unit IS one of the measure, so
-- the jedinična cena equals the prodajna cena. Without this column the unit price
-- of anything sold by package could only ever be a copy of the sale price, which
-- is exactly the defect req. 10 names. A sadržaj without its measure divides by
-- nothing and cannot produce a price, so the schema refuses that half-state.
ALTER TABLE products ADD COLUMN jedinicna_cena_sadrzaj_milli INTEGER
    CHECK (jedinicna_cena_sadrzaj_milli IS NULL
           OR (jedinicna_cena_sadrzaj_milli > 0 AND jedinicna_cena_jedinica IS NOT NULL));

-- Req. 12 — the till-side price-integrity guard's record. Čl. 6 st. 4 binds a
-- trader who publishes a cenovnik to adhere to the prices in it, so an article
-- rung above its published price is a compliance event of exactly the kind the
-- čl. 46 AML entry already is: warned about at the till, never refused, and
-- recorded in the one never-deleted trail so an inspection can reproduce the
-- decision from the log alone. SQLite cannot alter a CHECK, so admitting the
-- event is a table rebuild in the style of v16's — which is what the ids, the
-- details and the index below are carried across for.
CREATE TABLE compliance_log_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('trading_data_reset', 'backup_restored', 'aml_cash_threshold', 'cenovnik_price_divergence')),
    detail_json TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
INSERT INTO compliance_log_next (id, event_type, detail_json, user_id, created_at)
SELECT id, event_type, detail_json, user_id, created_at FROM compliance_log;
DROP TABLE compliance_log;
ALTER TABLE compliance_log_next RENAME TO compliance_log;
CREATE INDEX idx_compliance_log_created_at ON compliance_log(created_at);
"#,
    },
    Migration {
        version: 20,
        name: "popis_stores_blind_count_and_posting_lock",
        sql: r#"
-- SW-16, the popis stores. Two scope answers shape this schema and neither is
-- obvious from the column list alone:
--
--   1. Pravilnik 89/2020 runs čl. 1–16 and ends at the minister's signature — no
--      prilog, no obrazac, no column list for the popisna lista. So the columns
--      below are OURS, and the SW-9c KEP print-fidelity constraints must NOT be
--      carried over. What the bylaw does prescribe is the PROCESS (čl. 8–9), the
--      separate liste (čl. 2 st. 5, čl. 10–12) and the content of the izveštaj
--      (čl. 13 st. 1) — which is why the two vocabularies here are closed and the
--      free-form fields around them are not.
--   2. Pravilnik 140/2004's prosto-knjigovodstvo column set is deliberately
--      absent: ZPDG čl. 40 st. 2 t. 2) excludes retail from paušal and čl. 43
--      st. 3 confines prosto knjigovodstvo to a poljoprivrednik or drugo lice, so
--      that regime is legally unavailable to this shop and shipping its columns
--      would invite it into a regime it cannot use.
CREATE TABLE popis_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- ZoRač čl. 20 (the annual popis at the balance date) and čl. 21 (the popis
    -- on a retail price change). Two modes, not one mode with a flag: they carry
    -- different izveštaj deadlines — 60 days before the FS filing deadline vs 30
    -- days after the count (PoP čl. 13 st. 2) — and a session whose kind is
    -- unknown cannot be given either.
    vrsta TEXT NOT NULL CHECK (vrsta IN ('godisnji', 'nivelacioni')),
    -- Čl. 21 attaches the nivelacija duty to a „maloprodajni objekat“ and the
    -- annual popis is taken per place of business, so a popis with no outlet is a
    -- count of nothing.
    prodajno_mesto TEXT NOT NULL CHECK (prodajno_mesto <> ''),
    -- The count date, and the anchor of three separate deadlines: the čl. 13 st. 2
    -- izveštaj rok, the čl. 14 st. 2 odluka o usvajanju, and the čl. 2 st. 6
    -- ten-day consignment copy. Every ISO date column in this schema carries the
    -- shape check for the same reason v17 gave: '2026-12-1' and '2026-12-01' are
    -- one day to a human and two keys to SQLite, and here the difference would be
    -- computed into a wrong statutory deadline.
    datum_popisa TEXT NOT NULL CHECK (datum_popisa GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    period_from TEXT CHECK (period_from IS NULL OR period_from GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    period_to TEXT CHECK (period_to IS NULL OR period_to GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    -- The whole module is this state machine, and the vocabulary is the bylaw's
    -- own sequence: counting (čl. 9 st. 1 t. 1) → counted_signed (the čl. 8 st. 5
    -- signature, which is what releases the book data) → computed (t. 3–6) →
    -- computed_signed (the čl. 9 st. 3 signature on the printed liste) → posted
    -- (čl. 14 st. 3). Closed, because the blind-count guard and the posting lock
    -- below both read this column: an unrecognised state would not be refused by
    -- either of them, it would simply not match, and the guards would pass.
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN (
        'draft', 'counting', 'counted_signed', 'computed', 'computed_signed', 'posted'
    )),
    -- PoP čl. 8 st. 1–2: a plan rada is a mandatory PRE-count artefact, approved
    -- by the lice iz čl. 4 st. 2 — for a preduzetnik, the owner personally
    -- (čl. 4 st. 2 → ZoRač čl. 43 st. 3).
    plan_rada_json TEXT,
    odluka_ref TEXT,
    -- ZoRač čl. 20 st. 3 legislates an ORDERING: glavna knjiga↔dnevnik and
    -- pomoćne knjige↔glavna knjiga are reconciled BEFORE the popis. RFC3339,
    -- supplied as `now: &str` like every other decision stamp in this schema.
    -- NULL means the gate has not been passed, which is the state a new session
    -- starts in — the refusal itself is the command layer's (req. 39).
    uskladjivanje_potvrdjeno_at TEXT CHECK (uskladjivanje_potvrdjeno_at IS NULL OR uskladjivanje_potvrdjeno_at <> ''),
    -- PoP čl. 9 st. 2's perpetual-inventory shortcut is a CONDITIONAL GATE, never
    -- a default (req. 34): it is available only where a completed, adopted and
    -- posted in-year popis can be pointed at, and it excuses step 2) alone. This
    -- column is that pointer; without it the shortcut is unavailable and a
    -- physical count is forced.
    perpetual_odluka_ref TEXT,
    posted_at TEXT,
    created_at TEXT NOT NULL CHECK (created_at <> ''),
    updated_at TEXT NOT NULL CHECK (updated_at <> ''),
    CHECK (period_from IS NULL OR period_to IS NULL OR period_to >= period_from),
    -- The posted state and its stamp are one fact. Half of it either way round
    -- would be a popis that claims a knjiženje it cannot date, or a knjiženje
    -- date on a popis that was never posted.
    CHECK ((status = 'posted') = (posted_at IS NOT NULL))
);
CREATE INDEX idx_popis_sessions_datum ON popis_sessions(datum_popisa);

CREATE TABLE popis_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES popis_sessions(id),
    -- Req. 36 — the posebne popisne liste are REQUIRED where the category is
    -- present, not optional extras: konsignaciona/tuđa roba (čl. 2 st. 5, whose
    -- signed copy must reach the owner within ten days of the count date, čl. 2
    -- st. 6) · oštećena, zastarela i neupotrebljiva roba (čl. 10 st. 3) · roba van
    -- objekta, uključujući na popravci i kod trećeg lica (čl. 10 st. 4) ·
    -- gotovina po apoenima (čl. 11 st. 1) · nedokumentovana potraživanja i obaveze
    -- (čl. 12 st. 2) — plus the ordinary roba lista. Closed, and ASCII-folded like
    -- every other stored vocabulary in this database.
    lista_vrsta TEXT NOT NULL CHECK (lista_vrsta IN (
        'roba', 'ostecena', 'van_objekta', 'gotovina', 'potrazivanja', 'konsignacija'
    )),
    -- PoP čl. 8 st. 4: the commission is handed liste with nomenklaturni broj,
    -- naziv, vrsta and jedinica mere BEFORE the count begins. Quantities are the
    -- one thing čl. 8 st. 5 keeps back, so these four are free to be pre-filled.
    -- Šifra is nullable: a gotovina or potraživanje line has no article number.
    sifra TEXT,
    naziv TEXT NOT NULL CHECK (naziv <> ''),
    vrsta TEXT,
    jedinica_mere TEXT,
    -- Čl. 9 st. 1 t. 1 — the natural count. Milli-units, like every quantity in
    -- this schema. No floor is needed on this one for correctness, but a count is
    -- the result of counting something and nobody counts minus three: a negative
    -- here is a data-entry accident, and admitting it would post a phantom višak.
    stvarna_kolicina_milli INTEGER NOT NULL DEFAULT 0 CHECK (stvarna_kolicina_milli >= 0),
    -- Čl. 9 st. 1 t. 1's „bliži opis“ — the condition, the location, the reason a
    -- line sits on a posebna lista.
    blizi_opis TEXT,
    -- Čl. 9 st. 1 t. 3, AND THE POINT OF THE WHOLE MODULE. Nullable because it is
    -- written at the Phase A→B transition and at no earlier moment: čl. 8 st. 5
    -- forbids giving book quantities to the commission before the counted state is
    -- written into the liste and signed. The trigger below is what makes that a
    -- property of the database rather than of a screen — during a count there is
    -- no book quantity stored for any query, report, export or backup to leak.
    --
    -- Deliberately NO non-negative floor, unlike the counted quantity above: an
    -- oversold ledger genuinely reads below zero, and that reading is the manjak
    -- the popis exists to surface.
    knjigovodstvena_kolicina_milli INTEGER,
    -- Čl. 9 st. 1 t. 5, integer minor units (para). Signed: a nedokumentovana
    -- obaveza on the čl. 12 st. 2 lista is a negative value, not a second table.
    cena_minor INTEGER,
    created_at TEXT NOT NULL CHECK (created_at <> ''),
    updated_at TEXT NOT NULL CHECK (updated_at <> '')
);
CREATE INDEX idx_popis_lines_session ON popis_lines(session_id, lista_vrsta);

-- Req. 30 — TWO distinct signature events, each freezing its own snapshot with
-- its own timestamp: čl. 8 st. 5 (faza 'a', the members sign the counted state
-- before any book data is released) and čl. 9 st. 3 (faza 'b', the computed liste
-- are PRINTED and signed — „uz štampanje“ is express, so print-and-sign is the
-- default compliant path and a purely electronic signature is an unverified
-- deviation that must never be presented in-product as compliant, §6 R-6).
--
-- No updated_at, by design: a potpis is an event at a moment, not a record kept
-- up to date. A second signing is a second row.
CREATE TABLE popis_signatures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES popis_sessions(id),
    faza TEXT NOT NULL CHECK (faza IN ('a', 'b')),
    potpisnik TEXT NOT NULL CHECK (potpisnik <> ''),
    potpisano_at TEXT NOT NULL CHECK (potpisano_at <> ''),
    -- What was signed. A signature with no snapshot behind it attests to nothing,
    -- and the čl. 8 st. 5 one in particular is the evidence that the counted state
    -- was fixed BEFORE the book quantities were written.
    snapshot_hash TEXT NOT NULL CHECK (snapshot_hash <> ''),
    created_at TEXT NOT NULL CHECK (created_at <> '')
);
CREATE INDEX idx_popis_signatures_session ON popis_signatures(session_id, faza);

-- Req. 40. PoP čl. 5 st. 1 keeps lica koja rukuju imovinom off the commission, and
-- čl. 6 st. 1–2 let a preduzetnik run the popis with a single person to whom the
-- commission rules apply „shodno“. Whether that shodna primena carries the čl. 5
-- st. 1 exclusion is UNRESOLVED (§6 R-5), so this table records the fact and the
-- product WARNS on it — it must never block. The flag is stored per named person
-- precisely so the warning can name who it is about.
CREATE TABLE popis_commission (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES popis_sessions(id),
    ime TEXT NOT NULL CHECK (ime <> ''),
    -- Closed: 'jedno_lice' is the čl. 6 st. 1 single-person popis, which is a
    -- distinct legal shape and not a commission of one.
    uloga TEXT NOT NULL DEFAULT 'clan' CHECK (uloga IN ('predsednik', 'clan', 'jedno_lice')),
    -- Defaults to 0: nobody is presumed to handle the goods being counted, and a
    -- warning that fired on everyone would be read as noise and clicked past.
    rukuje_imovinom INTEGER NOT NULL DEFAULT 0 CHECK (rukuje_imovinom IN (0, 1)),
    created_at TEXT NOT NULL CHECK (created_at <> '')
);
CREATE INDEX idx_popis_commission_session ON popis_commission(session_id);

-- PoP čl. 8 st. 5 — „Подаци из књиговодства, односно из одговарајућих евиденција о
-- количинама, не могу се давати комисији за попис пре уписивања стварног стања у
-- пописне листе и пре него што чланови комисије за попис потпишу те листе.“
--
-- This is req. 29 and the single biggest constraint in the module. A UI that
-- merely hides a column does not discharge it: the duty is about the data
-- REACHING the commission, and a hidden column is still in the row, the export,
-- the backup and every ad-hoc query. So the engine refuses to store one while the
-- session is in Phase A, and the blind count becomes a property of the database.
--
-- The release condition is the POTPIS, not the status column. `status` is a claim
-- any UPDATE can make — and a session can be born in 'counted_signed' — so a guard
-- keyed on the flag alone would hand the book data to a commission that never
-- signed anything, which is precisely the failure čl. 8 st. 5 names. The article
-- makes both facts conditions („пре уписивања стварног стања … и пре него што
-- чланови комисије … потпишу те листе“), so both are checked: the session must
-- have left the counting states AND a faza 'a' signature must exist. The čl. 9
-- st. 3 signature is a different event and does not stand in for it.
--
-- The two guards share one trigger per write event so each keeps its own message:
-- the posting lock (req. 41) is checked first, because on a posted popis nothing
-- may be written at all and „the popis is closed“ is the truer answer than „not
-- yet signed“.
CREATE TRIGGER trg_popis_lines_unos
BEFORE INSERT ON popis_lines
BEGIN
    SELECT CASE
        WHEN (SELECT status FROM popis_sessions WHERE id = NEW.session_id) = 'posted'
            THEN RAISE(ABORT, 'Proknjižen popis se ne menja — ispravka se sprovodi novim popisom.')
        WHEN NEW.knjigovodstvena_kolicina_milli IS NOT NULL
             AND ((SELECT status FROM popis_sessions WHERE id = NEW.session_id)
                      IN ('draft', 'counting')
                  OR NOT EXISTS (SELECT 1 FROM popis_signatures
                                  WHERE session_id = NEW.session_id AND faza = 'a'))
            THEN RAISE(ABORT, 'Knjigovodstvena količina ne sme da se upiše pre nego što se stvarno stanje unese u popisne liste i potpiše (PoP čl. 8 st. 5).')
    END;
END;
-- Both ends of the move are checked, not just the row's current home. An UPDATE
-- may rewrite `session_id`, so testing OLD alone would leave one way in: a line
-- created on an open session and then re-pointed at a posted one, which is adding
-- a line to a posted popis by another name.
CREATE TRIGGER trg_popis_lines_izmena
BEFORE UPDATE ON popis_lines
BEGIN
    SELECT CASE
        WHEN (SELECT status FROM popis_sessions WHERE id = OLD.session_id) = 'posted'
             OR (SELECT status FROM popis_sessions WHERE id = NEW.session_id) = 'posted'
            THEN RAISE(ABORT, 'Proknjižen popis se ne menja — ispravka se sprovodi novim popisom.')
        WHEN NEW.knjigovodstvena_kolicina_milli IS NOT NULL
             AND ((SELECT status FROM popis_sessions WHERE id = NEW.session_id)
                      IN ('draft', 'counting')
                  OR NOT EXISTS (SELECT 1 FROM popis_signatures
                                  WHERE session_id = NEW.session_id AND faza = 'a'))
            THEN RAISE(ABORT, 'Knjigovodstvena količina ne sme da se upiše pre nego što se stvarno stanje unese u popisne liste i potpiše (PoP čl. 8 st. 5).')
    END;
END;

-- Req. 41 / PoP čl. 14 st. 3 with ZoRač čl. 8 st. 4: once the result is posted the
-- liste, the commission and the izveštaj are evidence. A correction is a NEW
-- popis, never an edit of the posted one. The lock reads OLD.status, so the
-- transition INTO posted is the last permitted write and every one after it is
-- refused — including walking the status back.
CREATE TRIGGER trg_popis_sessions_zakljucan
BEFORE UPDATE ON popis_sessions
WHEN OLD.status = 'posted'
BEGIN
    SELECT RAISE(ABORT, 'Proknjižen popis se ne menja — ispravka se sprovodi novim popisom.');
END;
CREATE TRIGGER trg_popis_commission_unos
BEFORE INSERT ON popis_commission
WHEN (SELECT status FROM popis_sessions WHERE id = NEW.session_id) = 'posted'
BEGIN
    SELECT RAISE(ABORT, 'Sastav komisije je deo proknjiženog popisa i ne može da se dopunjuje.');
END;
CREATE TRIGGER trg_popis_commission_izmena
BEFORE UPDATE ON popis_commission
WHEN (SELECT status FROM popis_sessions WHERE id = OLD.session_id) = 'posted'
     OR (SELECT status FROM popis_sessions WHERE id = NEW.session_id) = 'posted'
BEGIN
    SELECT RAISE(ABORT, 'Sastav komisije je deo proknjiženog popisa i ne može da se menja.');
END;

-- The posting lock reaches the signatures too, and this is the INSERT that most
-- needs it: a potpis is the strongest evidence artefact in the module, so one
-- dated after the čl. 14 st. 3 knjiženje would attest to a state that was already
-- posted — the same defect as joining the commission after the fact.
CREATE TRIGGER trg_popis_signatures_unos
BEFORE INSERT ON popis_signatures
WHEN (SELECT status FROM popis_sessions WHERE id = NEW.session_id) = 'posted'
BEGIN
    SELECT RAISE(ABORT, 'Potpis se ne dodaje na proknjižen popis — ispravka se sprovodi novim popisom.');
END;

-- A potpis is frozen when it is taken, not when the popis is posted: restamping
-- one would move the čl. 8 st. 5 or čl. 9 st. 3 moment the entire two-phase
-- design turns on, and the čl. 8 st. 5 signature is the evidence that the counted
-- state was fixed before the book quantities existed.
CREATE TRIGGER trg_popis_signatures_nepromenljiv
BEFORE UPDATE ON popis_signatures
BEGIN
    SELECT RAISE(ABORT, 'Potpis na popisnoj listi je nepromenljiv — nov potpis je nov zapis.');
END;

-- No DELETE trigger on any of the four tables, on purpose. Req. 42 gives the
-- popisne liste and the izveštaj a five-year retention FLOOR (ZoRač čl. 28 st. 7,
-- counted from the last day of the business year per st. 9) — a floor, not a
-- „trajno“ duty — so the retention purge must be able to reach an expired popis.
-- Closing DELETE here would close the purge with it and put the product in
-- permanent breach of storage limitation, exactly as v18 reasoned for the audit
-- log and v19 for the cenovnik archive.
"#,
    },
    Migration {
        version: 21,
        name: "popis_plan_rada_approval_and_odluka_date",
        sql: r#"
-- Req. 35 / PoP čl. 8 st. 1–2. v20 stored the plan rada's text (plan_rada_json)
-- and a free-text reference to the odluka (odluka_ref), but neither carried the
-- ACT: čl. 8 st. 2 requires the plan to be approved by the lice iz čl. 4 st. 2 —
-- for a preduzetnik the owner personally (čl. 4 st. 2 → ZoRač čl. 43 st. 3) — and
-- with no record of who approved it and when, an approved plan and a merely typed
-- one read back identically.
--
-- All three columns are nullable and nothing is backfilled. An existing session
-- has no approval, and a popis may legitimately be opened before the plan is
-- approved; writing one in on the shop's behalf would record a čl. 8 st. 2 act
-- that nobody performed, which is a worse defect than the missing column.
ALTER TABLE popis_sessions ADD COLUMN plan_rada_odobrio TEXT
    CHECK (plan_rada_odobrio IS NULL OR plan_rada_odobrio <> '');
-- The pairing is the point. An approver with no date, or a date with no approver,
-- is not an approval — it is half a record of one, and čl. 8 st. 2 is satisfied
-- only by the whole. SQLite evaluates every CHECK on every write regardless of
-- which column the statement names, so this one constraint refuses both halves,
-- on INSERT and on UPDATE, in either direction (recording and withdrawing).
ALTER TABLE popis_sessions ADD COLUMN plan_rada_odobreno_at TEXT
    CHECK ((plan_rada_odobreno_at IS NULL OR plan_rada_odobreno_at <> '')
           AND (plan_rada_odobrio IS NULL) = (plan_rada_odobreno_at IS NULL));
-- The odluka o popisu i obrazovanju komisije is a separate act from the approval
-- of the plan rada, so its date stands alone rather than pairing with anything.
-- RFC3339 like every other decision stamp in this schema, supplied as `now: &str`.
ALTER TABLE popis_sessions ADD COLUMN odluka_doneta_at TEXT
    CHECK (odluka_doneta_at IS NULL OR odluka_doneta_at <> '');
-- No trigger work: v20's trg_popis_sessions_zakljucan fires BEFORE UPDATE on the
-- whole row (WHEN OLD.status = 'posted') rather than naming columns, so the
-- req. 41 posting lock already covers these three. That is asserted by test, not
-- assumed — an approval recorded after the čl. 14 st. 3 knjiženje would date a
-- čl. 8 st. 2 act to after the popis it was supposed to precede.
"#,
    },
    Migration {
        version: 22,
        name: "reklamacija_no_fee_attestation",
        sql: r#"
-- 35/2026 čl. 63 st. 3: „Zabranjeno je da trgovac naplaćuje utvrđivanje
-- nesaobraznosti." Attested once per complaint, NEW regime only — 88/2021
-- čl. 55 st. 3 carries no such ban, so old-regime rows stay 0 forever and that
-- 0 must never be read as a failure to comply with a duty that never bound them.
ALTER TABLE reklamacije ADD COLUMN no_fee_attested INTEGER NOT NULL DEFAULT 0
    CHECK (no_fee_attested IN (0, 1));
ALTER TABLE reklamacije ADD COLUMN no_fee_attested_at TEXT;
"#,
    },
    Migration {
        version: 23,
        name: "support_session_odsustvo_unmask_stamp",
        sql: r#"
-- SW-14 req. 28: remote support must not see the absence REASON by default, and
-- the shop's decision to reveal it holds „for that session“. So the decision is a
-- property of the ZZPL čl. 46 nalog — this table — and not a state derived by
-- reading `audit_events` back: v18 keeps that log a record and not an access
-- control, and reading it is deliberately not logged.
--
-- WHY A STAMP AND NOT A BOOLEAN. `kategorija_odsustva` is a čl. 17 posebna vrsta
-- podataka in all but name, and revealing it to the obrađivač is irreversible:
-- once the operator has read the column, flipping a flag back does not unsee it.
-- A boolean would let the surface above this table claim otherwise — „maskirano“
-- again, over data already disclosed — which is the false statement čl. 46 is
-- least able to afford. An instant can only ever record that the disclosure
-- happened and when, so there is no re-mask verb and no column for one.
--
-- Nullable and never backfilled, because masked is the default and most naloge
-- never unmask: NULL is „nobody revealed anything under this nalog“, and writing
-- an instant in on an existing session would put a disclosure on file that did
-- not happen. The stamp needs no expiry of its own — the nalog is already
-- hard-bounded at 24 h by `expires_at`, so the unmask dies with the session it
-- belongs to.
--
-- Non-empty because '' is not an instant; the same reasoning v21 applied to
-- plan_rada_odobreno_at. A stamp that cannot say WHEN is not a record of a
-- disclosure, it is only a rumour of one.
ALTER TABLE support_sessions ADD COLUMN odsustvo_otkriveno_at TEXT
    CHECK (odsustvo_otkriveno_at IS NULL OR odsustvo_otkriveno_at <> '');
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
    use crate::db::{remove_test_database, test_database_path, Db};

    fn table_exists(conn: &Connection, table: &str) -> bool {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                rusqlite::params![table],
                |row| row.get(0),
            )
            .expect("sqlite_master should query");
        count == 1
    }

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

    /// Pull the literals out of a `<column> IN ('a', 'b', …)` CHECK in a CREATE TABLE
    /// body. Returns an empty vector when no such CHECK is present, so a caller that
    /// asserts on the values fails loudly if the constraint is ever dropped rather
    /// than quietly asserting over nothing.
    fn closed_enum_values(schema: &str, column: &str) -> Vec<String> {
        let needle = format!("{column} IN (");
        let Some(start) = schema.find(&needle) else {
            return Vec::new();
        };
        let rest = &schema[start + needle.len()..];
        let Some(end) = rest.find(')') else {
            return Vec::new();
        };
        rest[..end]
            .split(',')
            .map(|value| value.trim().trim_matches('\'').to_string())
            .filter(|value| !value.is_empty())
            .collect()
    }

    /// Seed one popis session in a given state. `posted_at` follows the status
    /// because the schema binds the two: a session in any other state has not been
    /// posted, and a posted one carries the stamp that says when.
    fn seed_popis_session(conn: &Connection, id: i64, status: &str) {
        let posted_at: Option<&str> = if status == "posted" {
            Some("2027-01-15T18:00:00Z")
        } else {
            None
        };

        conn.execute(
            "INSERT INTO popis_sessions (id, vrsta, prodajno_mesto, datum_popisa, period_from,
                                         period_to, status, posted_at, created_at, updated_at)
             VALUES (?1, 'godisnji', 'Butik Centar', '2026-12-31', '2026-01-01', '2026-12-31',
                     ?2, ?3, '2026-12-31T08:00:00Z', '2026-12-31T08:00:00Z')",
            params![id, status, posted_at],
        )
        .unwrap_or_else(|error| panic!("a popis session in {status} should insert: {error}"));
    }

    /// Append one popisna lista line to a session, optionally carrying a book
    /// quantity — the payload PoP čl. 8 st. 5 governs.
    fn insert_popis_line(
        conn: &Connection,
        session_id: i64,
        naziv: &str,
        knjigovodstvena: Option<i64>,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO popis_lines (session_id, lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                                      stvarna_kolicina_milli, blizi_opis,
                                      knjigovodstvena_kolicina_milli, created_at, updated_at)
             VALUES (?1, 'roba', 'MAR-001', ?2, 'roba u prodavnici', 'kom', 7000,
                     'blago oštećena ambalaža', ?3,
                     '2026-12-31T09:00:00Z', '2026-12-31T09:00:00Z')",
            params![session_id, naziv, knjigovodstvena],
        )
    }

    /// Take the PoP čl. 8 st. 5 signature — the event that releases the book data.
    /// Phase B is anchored on THIS row existing rather than on the freely-settable
    /// `status` flag, so every test that expects a book quantity to be accepted has
    /// to sign first, which is the order the bylaw prescribes anyway.
    fn sign_popis_phase_a(conn: &Connection, session_id: i64) {
        conn.execute(
            "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                           snapshot_hash, created_at)
             VALUES (?1, 'a', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-a',
                     '2026-12-31T17:00:00Z')",
            params![session_id],
        )
        .unwrap_or_else(|error| panic!("the čl. 8 st. 5 signature should record: {error}"));
    }

    /// Count the book quantities a query could hand the commission for one session.
    /// Čl. 8 st. 5 is about the data REACHING them, so the assertion that matters is
    /// not „the write was refused“ but „there is nothing there to read“.
    fn stored_book_quantities(conn: &Connection, session_id: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM popis_lines
              WHERE session_id = ?1 AND knjigovodstvena_kolicina_milli IS NOT NULL",
            params![session_id],
            |row| row.get(0),
        )
        .expect("the blind-count check should query")
    }

    /// SW-16 Task 1 — the four popis stores. §2c settles that Pravilnik 89/2020
    /// prescribes NO obrazac for the popisna lista, so the column set is ours to
    /// design. The vocabularies are not: `status` is the čl. 8 / čl. 9 / čl. 14
    /// process itself, and `lista_vrsta` is the closed list of posebne popisne
    /// liste the bylaw requires (čl. 2 st. 5, čl. 10 st. 3, čl. 10 st. 4, čl. 11
    /// st. 1, čl. 12 st. 2). Both stay closed enums, because a popis read back as
    /// evidence cannot carry a state or a list kind nobody defined.
    #[test]
    fn migration_v20_adds_the_popis_stores() {
        let path = test_database_path("migration_v20_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for table in [
                "popis_sessions",
                "popis_lines",
                "popis_signatures",
                "popis_commission",
            ] {
                assert!(table_exists(&conn, table), "{table} should exist after v20");
            }

            for column in [
                "id",
                "vrsta",
                "prodajno_mesto",
                "datum_popisa",
                "period_from",
                "period_to",
                "status",
                "plan_rada_json",
                "odluka_ref",
                "uskladjivanje_potvrdjeno_at",
                "perpetual_odluka_ref",
                "posted_at",
                "created_at",
                "updated_at",
            ] {
                assert!(
                    column_exists(&conn, "popis_sessions", column),
                    "popis_sessions should carry {column}"
                );
            }

            for column in [
                "id",
                "session_id",
                "lista_vrsta",
                "sifra",
                "naziv",
                "vrsta",
                "jedinica_mere",
                "stvarna_kolicina_milli",
                "blizi_opis",
                "knjigovodstvena_kolicina_milli",
                "cena_minor",
                "created_at",
                "updated_at",
            ] {
                assert!(
                    column_exists(&conn, "popis_lines", column),
                    "popis_lines should carry {column}"
                );
            }

            for column in [
                "id",
                "session_id",
                "faza",
                "potpisnik",
                "potpisano_at",
                "snapshot_hash",
                "created_at",
            ] {
                assert!(
                    column_exists(&conn, "popis_signatures", column),
                    "popis_signatures should carry {column}"
                );
            }

            for column in [
                "id",
                "session_id",
                "ime",
                "uloga",
                "rukuje_imovinom",
                "created_at",
            ] {
                assert!(
                    column_exists(&conn, "popis_commission", column),
                    "popis_commission should carry {column}"
                );
            }

            // A signature is an event, not a record that is kept up to date: čl. 8
            // st. 5 and čl. 9 st. 3 each fix a moment, and a second signing is a
            // second row. The same holds for who was on the commission on the day.
            assert!(
                !column_exists(&conn, "popis_signatures", "updated_at"),
                "a potpis is never revised, so it must not carry an updated_at"
            );
            assert!(
                !column_exists(&conn, "popis_commission", "updated_at"),
                "commission membership is recorded, not maintained"
            );

            let sessions_schema: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'popis_sessions'",
                    [],
                    |row| row.get(0),
                )
                .expect("popis_sessions schema should load");
            assert_eq!(
                closed_enum_values(&sessions_schema, "status"),
                vec![
                    "draft",
                    "counting",
                    "counted_signed",
                    "computed",
                    "computed_signed",
                    "posted"
                ],
                "the six-state popis machine must be a closed CHECK, in process order"
            );
            assert_eq!(
                closed_enum_values(&sessions_schema, "vrsta"),
                vec!["godisnji", "nivelacioni"],
                "ZoRač čl. 20 (godišnji) and čl. 21 (nivelacija) are the two modes"
            );

            let lines_schema: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'popis_lines'",
                    [],
                    |row| row.get(0),
                )
                .expect("popis_lines schema should load");
            assert_eq!(
                closed_enum_values(&lines_schema, "lista_vrsta"),
                vec![
                    "roba",
                    "ostecena",
                    "van_objekta",
                    "gotovina",
                    "potrazivanja",
                    "konsignacija"
                ],
                "the six posebne popisne liste the bylaw names, and no seventh"
            );

            seed_popis_session(&conn, 1, "counting");

            // The Phase A line: a natural count with a bliži opis and NO book
            // quantity. That the column is nullable is the whole of čl. 9 st. 1
            // t. 3 being a LATER step than t. 1.
            insert_popis_line(&conn, 1, "Marama svilena", None)
                .expect("a blind count line should insert with no book quantity");
            let (stvarna, knjigovodstvena): (i64, Option<i64>) = conn
                .query_row(
                    "SELECT stvarna_kolicina_milli, knjigovodstvena_kolicina_milli
                       FROM popis_lines WHERE session_id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the counted line should read back");
            assert_eq!(stvarna, 7000, "quantities are milli-units, never floats");
            assert_eq!(
                knjigovodstvena, None,
                "knjigovodstvena_kolicina_milli is nullable — it is written at the \
                 Phase A→B transition and at no earlier moment"
            );

            // A counted quantity is the result of counting something; you cannot
            // count minus three. The book quantity carries no such floor on
            // purpose — an oversold ledger genuinely reads below zero, and that
            // negative IS the manjak the popis exists to surface.
            assert!(
                conn.execute(
                    "UPDATE popis_lines SET stvarna_kolicina_milli = -1000 WHERE session_id = 1",
                    [],
                )
                .is_err(),
                "a negative counted quantity is not a count"
            );

            for (column, value) in [
                ("vrsta", "'kvartalni'"),
                ("status", "'zakljucan'"),
                ("prodajno_mesto", "''"),
                ("datum_popisa", "'2026-12-1'"),
            ] {
                assert!(
                    conn.execute(
                        &format!("UPDATE popis_sessions SET {column} = {value} WHERE id = 1"),
                        [],
                    )
                    .is_err(),
                    "popis_sessions.{column} must refuse {value}"
                );
            }

            assert!(
                insert_popis_line(&conn, 1, "Marama", None).is_ok(),
                "a second line on the same lista is ordinary"
            );
            assert!(
                conn.execute(
                    "INSERT INTO popis_lines (session_id, lista_vrsta, naziv, stvarna_kolicina_milli,
                                              created_at, updated_at)
                     VALUES (1, 'sitan_inventar', 'Nešto', 1000,
                             '2026-12-31T09:00:00Z', '2026-12-31T09:00:00Z')",
                    [],
                )
                .is_err(),
                "the separate-list vocabulary is closed"
            );

            // The posting stamp and the posted state are one fact, so the schema
            // refuses to hold half of it either way round.
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET status = 'posted' WHERE id = 1",
                    [],
                )
                .is_err(),
                "a posted popis without its posting stamp is not posted"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET posted_at = '2027-01-15T18:00:00Z' WHERE id = 1",
                    [],
                )
                .is_err(),
                "a posting stamp on an unposted popis claims a knjiženje that never happened"
            );

            // Req. 40: the goods-handler flag is stored per named person and
            // defaults to „ne rukuje“ — the warning is the command layer's job
            // (warn, never block, §6 R-5), but the fact it warns on lives here.
            conn.execute(
                "INSERT INTO popis_commission (session_id, ime, uloga, rukuje_imovinom, created_at)
                 VALUES (1, 'Miloš Đurđević', 'predsednik', 1, '2026-12-30T08:00:00Z')",
                [],
            )
            .expect("a commission member should insert");
            conn.execute(
                "INSERT INTO popis_commission (session_id, ime, created_at)
                 VALUES (1, 'Jelena Šarić', '2026-12-30T08:00:00Z')",
                [],
            )
            .expect("a member with no explicit role should default");
            let (uloga, rukuje): (String, i64) = conn
                .query_row(
                    "SELECT uloga, rukuje_imovinom FROM popis_commission WHERE ime = 'Jelena Šarić'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the defaulted member should read back");
            assert_eq!(uloga, "clan");
            assert_eq!(
                rukuje, 0,
                "nobody is presumed to handle the goods being counted"
            );
            assert!(
                conn.execute(
                    "INSERT INTO popis_commission (session_id, ime, uloga, created_at)
                     VALUES (1, 'Neko', 'zapisnicar', '2026-12-30T08:00:00Z')",
                    [],
                )
                .is_err(),
                "the role vocabulary is closed"
            );

            // Čl. 6 st. 1 lets a preduzetnik run the popis with a single person,
            // and čl. 6 st. 2 applies the commission rules to that person shodno —
            // so „jedno lice“ is a role this schema can express, not an absence.
            conn.execute(
                "INSERT INTO popis_commission (session_id, ime, uloga, created_at)
                 VALUES (1, 'Vlasnik lično', 'jedno_lice', '2026-12-30T08:00:00Z')",
                [],
            )
            .expect("the single-person popis must be expressible");

            conn.execute(
                "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                 VALUES (1, 'a', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-a',
                         '2026-12-31T17:00:00Z')",
                [],
            )
            .expect("the čl. 8 st. 5 signature should record");
            assert!(
                conn.execute(
                    "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                                   snapshot_hash, created_at)
                     VALUES (1, 'c', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-c',
                             '2026-12-31T17:00:00Z')",
                    [],
                )
                .is_err(),
                "there are exactly two signature events, čl. 8 st. 5 and čl. 9 st. 3"
            );
            assert!(
                conn.execute(
                    "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                                   snapshot_hash, created_at)
                     VALUES (1, 'b', 'Miloš Đurđević', '2026-12-31T17:00:00Z', '',
                             '2026-12-31T17:00:00Z')",
                    [],
                )
                .is_err(),
                "a signature over no snapshot attests to nothing"
            );
        }
        remove_test_database(&path);
    }

    /// PoP čl. 8 st. 5 — „Подаци из књиговодства … о количинама, не могу се давати
    /// комисији за попис пре уписивања стварног стања у пописне листе и пре него
    /// што чланови комисије за попис потпишу те листе.“ This is the single biggest
    /// constraint in SW-16 (req. 29), and a UI that merely hides a column does not
    /// satisfy it. The engine refuses to STORE a book quantity while the session is
    /// in Phase A, so during a count there is no book quantity in the database for
    /// any query, report, export or backup to hand the commission.
    #[test]
    fn migration_v20_refuses_a_book_quantity_before_the_phase_a_signature() {
        let path = test_database_path("migration_v20_blind_count");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for (id, status) in [(1_i64, "draft"), (2, "counting")] {
                seed_popis_session(&conn, id, status);

                insert_popis_line(&conn, id, "Marama svilena", None).unwrap_or_else(|error| {
                    panic!("the count itself must work in {status}: {error}")
                });

                assert!(
                    insert_popis_line(&conn, id, "Marama svilena", Some(9000)).is_err(),
                    "a line carrying a book quantity must be refused in {status}"
                );
                assert!(
                    conn.execute(
                        "UPDATE popis_lines SET knjigovodstvena_kolicina_milli = 9000
                          WHERE session_id = ?1",
                        params![id],
                    )
                    .is_err(),
                    "nor may a book quantity be smuggled in by UPDATE in {status}"
                );

                assert_eq!(
                    stored_book_quantities(&conn, id),
                    0,
                    "in {status} no query may find a book quantity to hand the commission"
                );
            }

            // The status column is a CLAIM; the potpis is the EVIDENCE. A session
            // can be born in counted_signed or flipped into it by any UPDATE, so a
            // guard keyed on the flag alone would release the book data to a
            // commission that never signed anything — the exact failure čl. 8 st. 5
            // names. Session 3 is that attack: the flag says signed, the ledger of
            // signatures is empty.
            seed_popis_session(&conn, 3, "counted_signed");
            insert_popis_line(&conn, 3, "Marama svilena", None)
                .expect("the counted state is written before the signature, not after");
            assert!(
                insert_popis_line(&conn, 3, "Marama svilena", Some(9000)).is_err(),
                "a counted_signed flag with no potpis behind it must not release the book data"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_lines SET knjigovodstvena_kolicina_milli = 9000
                      WHERE session_id = 3",
                    [],
                )
                .is_err(),
                "nor may the unsigned flag be used to smuggle one in by UPDATE"
            );

            // And the čl. 9 st. 3 potpis is not the čl. 8 st. 5 one: only the Phase
            // A signature releases the quantities, so a faza 'b' row changes nothing.
            conn.execute(
                "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                 VALUES (3, 'b', 'Miloš Đurđević', '2026-12-31T17:30:00Z', 'hash-b',
                         '2026-12-31T17:30:00Z')",
                [],
            )
            .expect("a faza 'b' signature is an ordinary row");
            assert!(
                insert_popis_line(&conn, 3, "Marama svilena", Some(9000)).is_err(),
                "the čl. 9 st. 3 potpis does not stand in for the čl. 8 st. 5 one"
            );
            assert_eq!(
                stored_book_quantities(&conn, 3),
                0,
                "with no čl. 8 st. 5 potpis in existence there is nothing for any query, \
                 export or backup to hand the commission"
            );

            // Once the counted state is written and SIGNED, čl. 9 st. 1 t. 3 is the
            // very next step and the book quantities may be entered.
            sign_popis_phase_a(&conn, 3);
            insert_popis_line(&conn, 3, "Marama svilena", Some(9000))
                .expect("after the čl. 8 st. 5 signature the book quantity may be written");

            // And it may be negative. An oversold ledger reads below zero, and that
            // reading is the manjak the popis exists to find — a floor here would
            // silently refuse the very rows the module is for.
            insert_popis_line(&conn, 3, "Kaiš kožni", Some(-2000))
                .expect("a negative book quantity is a real ledger state, not a typo");

            assert_eq!(
                stored_book_quantities(&conn, 3),
                2,
                "phase B is where the book quantities live"
            );

            // The signature is necessary, not sufficient. Reopening a signed count
            // puts the session back into a counting state, and the guard reads the
            // CURRENT status too — so session 3, potpis and all, is blind again for
            // every subsequent write.
            conn.execute(
                "UPDATE popis_sessions SET status = 'counting' WHERE id = 3",
                [],
            )
            .expect("the state machine, not the schema, decides legal transitions");
            assert!(
                insert_popis_line(&conn, 3, "Torba", Some(1000)).is_err(),
                "the guard follows the session's current state, not the row's history"
            );
        }
        remove_test_database(&path);
    }

    /// Req. 41 / PoP čl. 14 st. 3 with ZoRač čl. 8 st. 4: once the popis result is
    /// posted, the liste, the commission and the signatures are evidence and stop
    /// being editable — a correction is a NEW popis, never an edit of the old one.
    /// DELETE stays open on every one of these tables, because req. 42 gives them a
    /// five-year retention FLOOR rather than a „trajno“ duty and the purge must be
    /// able to reach an expired popis. That is the same boundary v18 and v19 drew,
    /// and it is pinned here so the schema comment can never become a promise the
    /// engine does not keep.
    #[test]
    fn migration_v20_write_locks_a_posted_popis_but_leaves_delete_open() {
        let path = test_database_path("migration_v20_posting_lock");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            seed_popis_session(&conn, 1, "computed_signed");
            sign_popis_phase_a(&conn, 1);
            insert_popis_line(&conn, 1, "Marama svilena", Some(9000))
                .expect("a computed line should insert before posting");
            conn.execute(
                "INSERT INTO popis_commission (session_id, ime, uloga, rukuje_imovinom, created_at)
                 VALUES (1, 'Miloš Đurđević', 'predsednik', 0, '2026-12-30T08:00:00Z')",
                [],
            )
            .expect("a commission member should insert before posting");
            conn.execute(
                "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                 VALUES (1, 'b', 'Miloš Đurđević', '2027-01-10T17:00:00Z', 'hash-b',
                         '2027-01-10T17:00:00Z')",
                [],
            )
            .expect("the čl. 9 st. 3 signature should record");

            // A signature is frozen the moment it is taken — before posting, not
            // because of it. Restamping one would move the čl. 8 st. 5 / čl. 9
            // st. 3 moment the whole two-phase design turns on.
            for (column, value) in [
                ("potpisano_at", "2027-02-01T09:00:00Z"),
                ("potpisnik", "Neko Drugi"),
                ("snapshot_hash", "hash-podmetnut"),
                ("faza", "a"),
            ] {
                assert!(
                    conn.execute(
                        &format!(
                            "UPDATE popis_signatures SET {column} = '{value}' WHERE faza = 'b'"
                        ),
                        [],
                    )
                    .is_err(),
                    "no path may rewrite {column} on a potpis"
                );
            }

            conn.execute(
                "UPDATE popis_sessions
                    SET status = 'posted', posted_at = '2027-01-15T18:00:00Z',
                        updated_at = '2027-01-15T18:00:00Z'
                  WHERE id = 1",
                [],
            )
            .expect("posting is the last permitted write");

            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET updated_at = '2027-02-01T09:00:00Z' WHERE id = 1",
                    [],
                )
                .is_err(),
                "a posted popis session is frozen, down to its updated_at"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET status = 'computed' , posted_at = NULL WHERE id = 1",
                    [],
                )
                .is_err(),
                "posting cannot be undone by walking the status back"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_lines SET stvarna_kolicina_milli = 1000 WHERE session_id = 1",
                    [],
                )
                .is_err(),
                "a counted quantity on a posted popis is evidence, not a draft"
            );
            assert!(
                insert_popis_line(&conn, 1, "Naknadno nađena marama", Some(1000)).is_err(),
                "an article remembered after posting belongs on a new popis"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_commission SET ime = 'Neko Drugi' WHERE session_id = 1",
                    [],
                )
                .is_err(),
                "who counted is part of what was posted"
            );
            assert!(
                conn.execute(
                    "INSERT INTO popis_commission (session_id, ime, created_at)
                     VALUES (1, 'Naknadni član', '2027-02-01T09:00:00Z')",
                    [],
                )
                .is_err(),
                "the commission cannot be joined after the result is posted"
            );
            // A potpis is the strongest evidence artefact in the module, so it is
            // the one INSERT that most needs the lock: a signature dated after the
            // čl. 14 st. 3 knjiženje would attest to a state that was already
            // posted — the joined-after-posting defect by another name.
            assert!(
                conn.execute(
                    "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                                   snapshot_hash, created_at)
                     VALUES (1, 'b', 'Naknadni potpisnik', '2027-03-01T10:00:00Z', 'hash-x',
                             '2027-03-01T10:00:00Z')",
                    [],
                )
                .is_err(),
                "a popisna lista cannot be signed after the result is posted"
            );

            // A second, unposted popis is unaffected — the lock is per session, not
            // a global freeze, and a correction goes through exactly this route.
            seed_popis_session(&conn, 2, "counting");
            insert_popis_line(&conn, 2, "Marama svilena", None)
                .expect("the corrective popis is an ordinary open session");
            conn.execute(
                "INSERT INTO popis_commission (session_id, ime, created_at)
                 VALUES (2, 'Jelena Šarić', '2027-02-01T09:00:00Z')",
                [],
            )
            .expect("the corrective popis gets its own commission");

            // The way in that testing only the row's current session would leave
            // open: `session_id` is itself updatable, so a line or a member raised
            // on the open corrective popis could be re-pointed at the posted one.
            // That is adding to a posted popis under another name, and both ends of
            // the move are checked.
            assert!(
                conn.execute(
                    "UPDATE popis_lines SET session_id = 1 WHERE session_id = 2",
                    []
                )
                .is_err(),
                "a line may not be moved onto a posted popis"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_commission SET session_id = 1 WHERE session_id = 2",
                    [],
                )
                .is_err(),
                "a member may not be moved onto a posted popis"
            );

            // Req. 42's five-year floor is a floor, not a „trajno“ duty: the purge
            // must be able to reach an expired popis, so DELETE stays open — in the
            // referential order the purge would have to use.
            conn.execute("DELETE FROM popis_lines WHERE session_id = 1", [])
                .expect("an expired popis lista must remain purgeable");
            conn.execute("DELETE FROM popis_commission WHERE session_id = 1", [])
                .expect("an expired commission record must remain purgeable");
            conn.execute("DELETE FROM popis_signatures WHERE session_id = 1", [])
                .expect("an expired signature must remain purgeable");
            conn.execute("DELETE FROM popis_sessions WHERE id = 1", [])
                .expect("an expired popis session must remain purgeable");
        }
        remove_test_database(&path);
    }

    /// The installed-base path: a till already running v19 gets the popis stores
    /// without losing a product, a price, a published cenovnik or an audit row.
    /// Seeding through a raw connection at MIGRATIONS[..19] and only then opening
    /// with `Db::new` is what makes this provable — seeding after `Db::new` would
    /// prove a new-schema round trip and nothing about the upgrade (commit
    /// 270796c).
    #[test]
    fn migration_v20_preserves_pre_existing_rows() {
        let path = test_database_path("migration_v20_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..19] {
                assert!(
                    migration.version <= 19,
                    "the pre-v20 prefix must stop at v19, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-08-01T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (200, 'PDV 20', 2000, '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO products (id, name, sku, barcode, unit_of_measure, sale_price_minor,
                                       purchase_price_minor, tax_rate_id, minimum_stock_milli,
                                       created_at, updated_at)
                     VALUES (200, 'Marama svilena', 'MAR-001', '0123456789012', 'kom', 249900,
                             120000, 200, 0, '2026-07-01T08:00:00Z', '2026-07-20T08:00:00Z');
                 INSERT INTO price_history (id, product_id, effective_from, price_minor, source, created_at)
                     VALUES (200, 200, '2026-07-20T08:00:00Z', 249900, 'update', '2026-07-20T08:00:00Z');
                 INSERT INTO cenovnik_snapshots (id, prodajno_mesto, generated_at, row_count,
                                                 content_hash, body, created_at)
                     VALUES (200, 'Butik Centar', '2026-07-20T08:00:05Z', 1, 'h-200',
                             'telo cenovnika', '2026-07-20T08:00:05Z');
                 INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (200, 'stari_kasir', 'Stari Kasir', 'cashier', 1,
                             '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO compliance_log (id, event_type, detail_json, user_id, created_at)
                     VALUES (200, 'cenovnik_price_divergence', '{\"snapshot_id\":200}', 200,
                             '2026-07-25T11:00:00Z');",
            )
            .expect("seed v19-era rows");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (name, barcode, price, updated): (String, String, i64, String) = conn
                .query_row(
                    "SELECT name, barcode, sale_price_minor, updated_at FROM products WHERE id = 200",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the pre-v20 product must survive verbatim");
            assert_eq!(name, "Marama svilena");
            assert_eq!(
                barcode, "0123456789012",
                "the leading zero of a 13-digit barcode must survive as text"
            );
            assert_eq!(price, 249900);
            assert_eq!(updated, "2026-07-20T08:00:00Z");

            let (history_product, history_price): (i64, i64) = conn
                .query_row(
                    "SELECT product_id, price_minor FROM price_history WHERE id = 200",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the price_history row must survive with its id");
            assert_eq!((history_product, history_price), (200, 249900));

            let (outlet, body): (String, String) = conn
                .query_row(
                    "SELECT prodajno_mesto, body FROM cenovnik_snapshots WHERE id = 200",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the published cenovnik must survive byte for byte");
            assert_eq!(
                (outlet.as_str(), body.as_str()),
                ("Butik Centar", "telo cenovnika")
            );

            let (event_type, detail): (String, String) = conn
                .query_row(
                    "SELECT event_type, detail_json FROM compliance_log WHERE id = 200",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the never-deleted trail must survive with its id");
            assert_eq!(
                (event_type.as_str(), detail.as_str()),
                ("cenovnik_price_divergence", "{\"snapshot_id\":200}")
            );

            for table in [
                "popis_sessions",
                "popis_lines",
                "popis_signatures",
                "popis_commission",
            ] {
                assert!(
                    table_exists(&conn, table),
                    "{table} should arrive on an upgraded database"
                );
                let rows: i64 = conn
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .unwrap_or_else(|error| panic!("{table} should count: {error}"));
                assert_eq!(
                    rows, 0,
                    "an upgrade opens no popis on its own — a popis is an act, not a backfill"
                );
            }

            // The upgrade must not have quietly rebuilt the v19 archive: its
            // immutability trigger is still the thing standing between a published
            // cenovnik and a rewritten one.
            assert!(
                conn.execute(
                    "UPDATE cenovnik_snapshots SET body = 'izmenjeno' WHERE id = 200",
                    [],
                )
                .is_err(),
                "the v19 immutability trigger must survive the v20 upgrade"
            );
        }
        remove_test_database(&path);
    }

    /// SW-16 follow-on, req. 35 / PoP čl. 8 st. 1–2: the plan rada is not merely
    /// written, it is APPROVED by the lice iz čl. 4 st. 2 — for a preduzetnik the
    /// owner personally (čl. 4 st. 2 → ZoRač čl. 43 st. 3) — and the odluka o
    /// popisu i obrazovanju komisije is issued before the count. v20 stored the
    /// plan's text and a free-text reference to the odluka, so an approved plan
    /// and a merely typed one read back identically.
    ///
    /// The pairing CHECK is the load-bearing part: an approver with no date, or a
    /// date with no approver, is not an approval, and a half-recorded one is the
    /// false-record class this project keeps catching.
    #[test]
    fn migration_v21_records_the_plan_rada_approval_and_the_odluka_date() {
        let path = test_database_path("migration_v21_plan_approval");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in [
                "plan_rada_odobrio",
                "plan_rada_odobreno_at",
                "odluka_doneta_at",
            ] {
                assert!(
                    column_exists(&conn, "popis_sessions", column),
                    "popis_sessions should carry {column} after v21"
                );
            }

            // A popis may legitimately be opened before the plan is approved, and
            // nothing approves it on the session's behalf: an approval nobody
            // performed would be a false record of a čl. 8 st. 2 act.
            seed_popis_session(&conn, 1, "draft");
            let (odobrio, odobreno_at, odluka_at): (
                Option<String>,
                Option<String>,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT plan_rada_odobrio, plan_rada_odobreno_at, odluka_doneta_at
                       FROM popis_sessions WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the fresh session should read back");
            assert_eq!(
                (odobrio, odobreno_at, odluka_at),
                (None, None, None),
                "a new popis carries no approval — čl. 8 st. 2 is an act, not a default"
            );

            // Each half-state, by UPDATE and by INSERT. Both directions matter:
            // a session can be born half-approved just as easily as edited into it.
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET plan_rada_odobrio = 'Miloš Đurđević' WHERE id = 1",
                    [],
                )
                .is_err(),
                "an approver with no date is not a čl. 8 st. 2 approval"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET plan_rada_odobreno_at = '2026-12-29T09:00:00Z'
                      WHERE id = 1",
                    [],
                )
                .is_err(),
                "a date with no approver is not a čl. 8 st. 2 approval"
            );
            assert!(
                conn.execute(
                    "INSERT INTO popis_sessions (id, vrsta, prodajno_mesto, datum_popisa, status,
                                                 plan_rada_odobrio, created_at, updated_at)
                     VALUES (9, 'godisnji', 'Butik Centar', '2026-12-31', 'draft',
                             'Miloš Đurđević', '2026-12-29T09:00:00Z', '2026-12-29T09:00:00Z')",
                    [],
                )
                .is_err(),
                "nor may a session be born half-approved"
            );

            // Blank is the half-state wearing a value: an empty approver names
            // nobody and an empty stamp dates nothing.
            assert!(
                conn.execute(
                    "UPDATE popis_sessions
                        SET plan_rada_odobrio = '', plan_rada_odobreno_at = '2026-12-29T09:00:00Z'
                      WHERE id = 1",
                    [],
                )
                .is_err(),
                "an empty approver names nobody"
            );
            assert!(
                conn.execute(
                    "UPDATE popis_sessions
                        SET plan_rada_odobrio = 'Miloš Đurđević', plan_rada_odobreno_at = ''
                      WHERE id = 1",
                    [],
                )
                .is_err(),
                "an empty approval stamp dates nothing"
            );

            // The whole approval, recorded in one write, is ordinary.
            conn.execute(
                "UPDATE popis_sessions
                    SET plan_rada_odobrio = 'Miloš Đurđević',
                        plan_rada_odobreno_at = '2026-12-29T09:00:00Z',
                        updated_at = '2026-12-29T09:00:00Z'
                  WHERE id = 1",
                [],
            )
            .expect("an approver and a date together are an approval");
            let (odobrio, odobreno_at): (String, String) = conn
                .query_row(
                    "SELECT plan_rada_odobrio, plan_rada_odobreno_at FROM popis_sessions
                      WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the approval should read back");
            assert_eq!(
                (odobrio.as_str(), odobreno_at.as_str()),
                ("Miloš Đurđević", "2026-12-29T09:00:00Z")
            );

            // And withdrawing it is the same rule in reverse: both halves go, or
            // neither does.
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET plan_rada_odobreno_at = NULL WHERE id = 1",
                    [],
                )
                .is_err(),
                "an approval cannot be half-withdrawn either"
            );

            // The odluka date stands on its own: čl. 8 st. 2's approval of the plan
            // and the odluka o popisu i obrazovanju komisije are separate acts.
            conn.execute(
                "UPDATE popis_sessions SET odluka_doneta_at = '2026-12-20T10:00:00Z' WHERE id = 1",
                [],
            )
            .expect("the odluka date is independent of the plan approval");
            assert!(
                conn.execute(
                    "UPDATE popis_sessions SET odluka_doneta_at = '' WHERE id = 1",
                    [],
                )
                .is_err(),
                "an empty odluka stamp says the odluka was issued at no time"
            );

            // Req. 41 — the posting lock reaches the new columns, because v20's
            // trigger freezes the whole row rather than naming columns. An approval
            // recorded after the čl. 14 st. 3 knjiženje would date a čl. 8 st. 2 act
            // to after the popis it was supposed to precede.
            seed_popis_session(&conn, 2, "posted");
            for assignment in [
                "plan_rada_odobrio = 'Neko', plan_rada_odobreno_at = '2027-02-01T09:00:00Z'",
                "odluka_doneta_at = '2027-02-01T09:00:00Z'",
            ] {
                let error = conn
                    .execute(
                        &format!("UPDATE popis_sessions SET {assignment} WHERE id = 2"),
                        [],
                    )
                    .expect_err("a posted popis takes no further approval");
                assert!(
                    error.to_string().contains("Proknjižen popis"),
                    "the posting lock, not a CHECK, must be what refuses {assignment}: {error}"
                );
            }
        }
        remove_test_database(&path);
    }

    /// The installed-base path: a till already running v20 gains the čl. 8 st. 2
    /// approval columns without losing a popis, a counted line or a potpis — and
    /// without being handed an approval it never performed. Seeding through a raw
    /// connection at `MIGRATIONS[..20]` and only then opening with `Db::new` is what
    /// makes this provable; seeding after `Db::new` would prove a new-schema round
    /// trip and nothing about the upgrade (commit 270796c).
    #[test]
    fn migration_v21_preserves_pre_existing_rows() {
        let path = test_database_path("migration_v21_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..20] {
                assert!(
                    migration.version <= 20,
                    "the pre-v21 prefix must stop at v20, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-08-02T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (210, 'PDV 20', 2000, '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO products (id, name, sku, barcode, unit_of_measure, sale_price_minor,
                                       purchase_price_minor, tax_rate_id, minimum_stock_milli,
                                       created_at, updated_at)
                     VALUES (210, 'Marama svilena', 'MAR-001', '0123456789012', 'kom', 249900,
                             120000, 210, 0, '2026-07-01T08:00:00Z', '2026-07-20T08:00:00Z');
                 INSERT INTO popis_sessions (id, vrsta, prodajno_mesto, datum_popisa, period_from,
                                             period_to, status, plan_rada_json, odluka_ref,
                                             uskladjivanje_potvrdjeno_at, created_at, updated_at)
                     VALUES (210, 'godisnji', 'Butik Centar', '2026-12-31', '2026-01-01',
                             '2026-12-31', 'computed', '{\"smene\":\"08-16\"}', 'Odluka 3/2026',
                             '2026-12-30T07:00:00Z', '2026-12-31T08:00:00Z', '2026-12-31T18:00:00Z');
                 INSERT INTO popis_commission (id, session_id, ime, uloga, rukuje_imovinom, created_at)
                     VALUES (210, 210, 'Miloš Đurđević', 'predsednik', 0, '2026-12-30T08:00:00Z');
                 INSERT INTO popis_signatures (id, session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                     VALUES (210, 210, 'a', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-a',
                             '2026-12-31T17:00:00Z');
                 INSERT INTO popis_lines (id, session_id, lista_vrsta, sifra, naziv, vrsta,
                                          jedinica_mere, stvarna_kolicina_milli, blizi_opis,
                                          knjigovodstvena_kolicina_milli, cena_minor,
                                          created_at, updated_at)
                     VALUES (210, 210, 'roba', 'MAR-001', 'Marama svilena', 'roba u prodavnici',
                             'kom', 7000, 'blago oštećena ambalaža', 9000, 249900,
                             '2026-12-31T09:00:00Z', '2026-12-31T17:30:00Z');",
            )
            .expect("seed v20-era rows");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (name, barcode, price): (String, String, i64) = conn
                .query_row(
                    "SELECT name, barcode, sale_price_minor FROM products WHERE id = 210",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the pre-v21 product must survive verbatim");
            assert_eq!(
                (name.as_str(), barcode.as_str(), price),
                ("Marama svilena", "0123456789012", 249900),
                "the leading zero of a 13-digit barcode must survive as text"
            );

            let (vrsta, mesto, datum, status, plan, odluka, uskladjivanje, created): (
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                String,
            ) = conn
                .query_row(
                    "SELECT vrsta, prodajno_mesto, datum_popisa, status, plan_rada_json,
                            odluka_ref, uskladjivanje_potvrdjeno_at, created_at
                       FROM popis_sessions WHERE id = 210",
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
                .expect("the pre-v21 popis session must survive verbatim");
            assert_eq!(
                (
                    vrsta.as_str(),
                    mesto.as_str(),
                    datum.as_str(),
                    status.as_str(),
                    plan.as_str(),
                    odluka.as_str(),
                    uskladjivanje.as_str(),
                    created.as_str(),
                ),
                (
                    "godisnji",
                    "Butik Centar",
                    "2026-12-31",
                    "computed",
                    "{\"smene\":\"08-16\"}",
                    "Odluka 3/2026",
                    "2026-12-30T07:00:00Z",
                    "2026-12-31T08:00:00Z",
                )
            );

            let (stvarna, knjigovodstvena, cena, opis): (i64, i64, i64, String) = conn
                .query_row(
                    "SELECT stvarna_kolicina_milli, knjigovodstvena_kolicina_milli, cena_minor,
                            blizi_opis
                       FROM popis_lines WHERE id = 210",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the counted line must survive the upgrade");
            assert_eq!(
                (stvarna, knjigovodstvena, cena, opis.as_str()),
                (7000, 9000, 249900, "blago oštećena ambalaža"),
                "quantities are milli-units and money is para — the upgrade rounds nothing"
            );

            let (potpisnik, potpisano_at, hash): (String, String, String) = conn
                .query_row(
                    "SELECT potpisnik, potpisano_at, snapshot_hash FROM popis_signatures
                      WHERE id = 210",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the čl. 8 st. 5 potpis must survive the upgrade");
            assert_eq!(
                (potpisnik.as_str(), potpisano_at.as_str(), hash.as_str()),
                ("Miloš Đurđević", "2026-12-31T17:00:00Z", "hash-a")
            );

            // The upgrade approves nothing. An approval nobody performed is exactly
            // the false record čl. 8 st. 2 is about, so a backfill here would be
            // worse than the missing column it replaced.
            let (odobrio, odobreno_at, odluka_doneta_at): (
                Option<String>,
                Option<String>,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT plan_rada_odobrio, plan_rada_odobreno_at, odluka_doneta_at
                       FROM popis_sessions WHERE id = 210",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the new columns should read back");
            assert_eq!(
                (odobrio, odobreno_at, odluka_doneta_at),
                (None, None, None),
                "an upgraded popis has an unapproved plan, not an approval it never got"
            );

            // The upgrade must not have quietly rebuilt the v20 table: the čl. 8
            // st. 5 blind-count trigger is still the thing standing between a
            // counting commission and the book quantities.
            seed_popis_session(&conn, 211, "counting");
            assert!(
                insert_popis_line(&conn, 211, "Marama svilena", Some(9000)).is_err(),
                "the v20 blind-count trigger must survive the v21 upgrade"
            );
        }
        remove_test_database(&path);
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

        remove_test_database(&path);
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

        remove_test_database(&path);
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
        remove_test_database(&path);
    }

    /// v15 is the first rebuild in this schema's history to touch tables that hold
    /// live money: `cash_movements` drives expected cash and the shortfall check, and
    /// `sale_payments` drives Z-reports, dnevni promet and KEP. An installed till is
    /// upgraded in place, so the copy step is the whole point of the rebuild — this
    /// test seeds at v14 and upgrades, which is the only way to prove it.
    #[test]
    fn migration_v22_adds_the_no_fee_attestation_columns() {
        let path = test_database_path("migration_v22_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in ["no_fee_attested", "no_fee_attested_at"] {
                assert!(
                    column_exists(&conn, "reklamacije", column),
                    "reklamacije.{column} should exist after v22"
                );
            }

            // A record predating the gate was never asked, so it must read as
            // unattested rather than as silently compliant.
            conn.execute(
                "INSERT INTO reklamacije (
                    register_number, regime, status, filed_at, podnosilac_ime_prezime,
                    podaci_o_robi, opis_nesaobraznosti, zahtev, roba_kind,
                    datum_izdavanja_potvrde, created_at, updated_at
                 )
                 VALUES (901, 'new', 'open', '2026-08-15T00:00:00Z', 'Petar Petrović',
                         'Veš mašina', 'Ne centrifugira', 'Popravka', 'tehnicka',
                         '2026-08-15T10:00:00Z', '2026-08-15T10:00:00Z', '2026-08-15T10:00:00Z')",
                [],
            )
            .expect("a reklamacija row should insert under v22");

            let (attested, attested_at): (i64, Option<String>) = conn
                .query_row(
                    "SELECT no_fee_attested, no_fee_attested_at FROM reklamacije
                     WHERE register_number = 901",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the seeded row should read back");
            assert_eq!(attested, 0, "the default must be UNattested");
            assert!(attested_at.is_none());
        }
        remove_test_database(&path);
    }

    /// SW-14 req. 28. The nalog gains the instant at which the shop revealed the
    /// absence reason for that session, and a freshly issued nalog carries none:
    /// masked is the default, so an upgraded or newly granted čl. 46 nalog reads
    /// back NULL rather than as a disclosure nobody made.
    ///
    /// The empty string is refused for the same reason `plan_rada_odobreno_at`
    /// refuses it in v21 — `''` is not an instant, and a stamp that cannot say
    /// WHEN the disclosure happened is not a record of one.
    #[test]
    fn migration_v23_adds_the_absence_reason_unmask_stamp() {
        let path = test_database_path("migration_v23_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            assert!(
                column_exists(&conn, "support_sessions", "odsustvo_otkriveno_at"),
                "support_sessions.odsustvo_otkriveno_at should exist after v23"
            );

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (930, 'vlasnik', 'Vlasnik Vlasnikovič', 'admin', 1,
                             '2026-08-09T09:00:00Z', '2026-08-09T09:00:00Z');
                 INSERT INTO support_sessions (id, granted_by, granted_at, scope, expires_at,
                                               created_at, updated_at)
                     VALUES (930, 930, '2026-08-09T09:00:00Z',
                             'Pregled greške na štampi fiskalnog isečka',
                             '2026-08-09T10:00:00Z', '2026-08-09T09:00:00Z',
                             '2026-08-09T09:00:00Z');",
            )
            .expect("a čl. 46 nalog should insert under v23");

            let stamp: Option<String> = conn
                .query_row(
                    "SELECT odsustvo_otkriveno_at FROM support_sessions WHERE id = 930",
                    [],
                    |row| row.get(0),
                )
                .expect("the new column should read back");
            assert_eq!(
                stamp, None,
                "the default is masked — a nalog nobody unmasked carries no stamp"
            );

            assert!(
                conn.execute(
                    "UPDATE support_sessions SET odsustvo_otkriveno_at = '' WHERE id = 930",
                    [],
                )
                .is_err(),
                "the empty string is not an instant — the CHECK must refuse it"
            );

            conn.execute(
                "UPDATE support_sessions SET odsustvo_otkriveno_at = '2026-08-09T09:30:00Z'
                   WHERE id = 930",
                [],
            )
            .expect("an RFC3339 instant should stamp");

            let stamped: Option<String> = conn
                .query_row(
                    "SELECT odsustvo_otkriveno_at FROM support_sessions WHERE id = 930",
                    [],
                    |row| row.get(0),
                )
                .expect("the stamp should read back");
            assert_eq!(stamped.as_deref(), Some("2026-08-09T09:30:00Z"));
        }
        remove_test_database(&path);
    }

    /// The installed-base path: a till already running v22 gains the req. 28 stamp
    /// without losing a nalog it issued or an absence it recorded. Seeding through
    /// a raw connection at `MIGRATIONS[..22]` and only then opening with `Db::new`
    /// is what makes this provable; seeding after `Db::new` would prove a
    /// new-schema round trip and nothing at all about the upgrade (commit 270796c).
    #[test]
    fn migration_v23_preserves_pre_existing_rows() {
        let path = test_database_path("migration_v23_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..22] {
                assert!(
                    migration.version <= 22,
                    "the pre-v23 prefix must stop at v22, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-08-08T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            // A nalog that was issued AND entered — the state in which a support
            // operator is actually on the machine — plus the one absence row whose
            // reason req. 28 exists to keep from them.
            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (930, 'vlasnik', 'Vlasnik Vlasnikovič', 'admin', 1,
                             '2026-08-01T07:00:00Z', '2026-08-01T07:00:00Z');
                 INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (931, 'prodavac', 'Prodavac Prodavčević', 'cashier', 1,
                             '2026-08-01T07:00:00Z', '2026-08-01T07:00:00Z');
                 INSERT INTO support_sessions (id, granted_by, granted_at, scope, expires_at,
                                               started_at, created_at, updated_at)
                     VALUES (930, 930, '2026-08-08T09:00:00Z',
                             'Pregled greške na štampi fiskalnog isečka',
                             '2026-08-08T10:00:00Z', '2026-08-08T09:05:00Z',
                             '2026-08-08T09:00:00Z', '2026-08-08T09:05:00Z');
                 INSERT INTO work_time_entries (id, user_id, dan, moguci_minuta,
                                                ukupno_neizvrseni_minuta, sprecenost_rfzo_minuta,
                                                kategorija_odsustva, created_at, updated_at)
                     VALUES (930, 931, '2026-08-07', 480, 480, 480, 'sprecenost_rfzo',
                             '2026-08-07T18:00:00Z', '2026-08-07T18:00:00Z');",
            )
            .expect("seed v22-era rows");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (granted_by, granted_at, scope, expires_at, started_at, created_at, updated_at): (
                i64,
                String,
                String,
                String,
                String,
                String,
                String,
            ) = conn
                .query_row(
                    "SELECT granted_by, granted_at, scope, expires_at, started_at, created_at,
                            updated_at
                       FROM support_sessions WHERE id = 930",
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
                        ))
                    },
                )
                .expect("the pre-v23 nalog must survive verbatim");
            assert_eq!(
                (
                    granted_by,
                    granted_at.as_str(),
                    scope.as_str(),
                    expires_at.as_str(),
                    started_at.as_str(),
                    created_at.as_str(),
                    updated_at.as_str(),
                ),
                (
                    930,
                    "2026-08-08T09:00:00Z",
                    "Pregled greške na štampi fiskalnog isečka",
                    "2026-08-08T10:00:00Z",
                    "2026-08-08T09:05:00Z",
                    "2026-08-08T09:00:00Z",
                    "2026-08-08T09:05:00Z",
                ),
                "the nalog is the čl. 46 evidence — the upgrade rewrites no word of it"
            );

            let (ended_at, revoked_at): (Option<String>, Option<String>) = conn
                .query_row(
                    "SELECT ended_at, revoked_at FROM support_sessions WHERE id = 930",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the lifecycle stamps should read back");
            assert_eq!(
                (ended_at, revoked_at),
                (None, None),
                "the upgrade neither ends nor revokes a live nalog"
            );

            // The upgrade discloses nothing. A stamp records an irreversible
            // disclosure, so backfilling one would put on file a čl. 46 session in
            // which the shop revealed the absence reason — an event that never
            // happened, and the one thing worse here than a missing column.
            let stamp: Option<String> = conn
                .query_row(
                    "SELECT odsustvo_otkriveno_at FROM support_sessions WHERE id = 930",
                    [],
                    |row| row.get(0),
                )
                .expect("the new column should read back");
            assert_eq!(
                stamp, None,
                "an upgraded nalog has unmasked nothing, and must not claim it did"
            );

            // v23 masks a READ. It deletes no column and rewrites no absence: the
            // ZoR čl. 55 register still holds the reason it always held.
            let (kategorija, neizvrseni, rfzo): (Option<String>, i64, i64) = conn
                .query_row(
                    "SELECT kategorija_odsustva, ukupno_neizvrseni_minuta, sprecenost_rfzo_minuta
                       FROM work_time_entries WHERE id = 930",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the pre-v23 absence must survive verbatim");
            assert_eq!(
                (kategorija.as_deref(), neizvrseni, rfzo),
                (Some("sprecenost_rfzo"), 480, 480),
                "minutes are whole minutes and the category is untouched by v23"
            );

            // The CHECK arrives with the column rather than only on a fresh
            // database, and the v18 constraints that make the row a nalog are still
            // standing — the upgrade added a column, it did not rebuild the table.
            assert!(
                conn.execute(
                    "UPDATE support_sessions SET odsustvo_otkriveno_at = '' WHERE id = 930",
                    [],
                )
                .is_err(),
                "the empty-stamp CHECK must bind an upgraded database too"
            );
            assert!(
                conn.execute(
                    "INSERT INTO support_sessions (granted_by, granted_at, scope, expires_at,
                                                   created_at, updated_at)
                     VALUES (930, '2026-08-08T09:00:00Z', 'Bez roka', '2026-08-08T08:00:00Z',
                             '2026-08-08T09:00:00Z', '2026-08-08T09:00:00Z')",
                    [],
                )
                .is_err(),
                "the v18 expiry CHECK must survive the v23 upgrade"
            );
        }
        remove_test_database(&path);
    }

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

        remove_test_database(&path);
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
        remove_test_database(&path);
    }

    /// v16 rebuilds `compliance_log` — the never-deleted audit trail whose rows are
    /// the evidence that a trading-data reset or a backup restore happened at all.
    /// An installed till is upgraded in place, so the copy step is the whole point
    /// of the rebuild; seeding at v15 and upgrading is the only way to prove it.
    /// The same upgrade adds `cash_movements.documented_per_pravilnik`, which must
    /// arrive NULL on every carried-forward movement: NULL means the operator has
    /// not asserted anything, and the exclusion it gates must default OFF.
    ///
    /// The seeded database is migrated all the way to head, so this also covers
    /// **v19's second rebuild** of the same table for the SW-12 till-guard event.
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

        remove_test_database(&path);
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
        remove_test_database(&path);
    }

    #[test]
    fn migration_v17_adds_worktime_schema() {
        let path = test_database_path("migration_v17_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for column in [
                "datum_rodjenja",
                "datum_rodjenja_najmladjeg_deteta",
                "samohrani_roditelj",
                // ZoR čl. 91 st. 2 has two legs; the težak-invalid leg has no age limit.
                "dete_tezak_invalid",
                "trudnoca_ili_dojenje",
                "trudnoca_ili_dojenje_od",
                "radi_u_preraspodeli",
                "ugovoreno_radno_vreme_minuta_nedeljno",
                "zanimanje_sifra",
                "kvalifikacija_sifra",
                // Two legally distinct written consents — čl. 91 and čl. 57 st. 4.
                "saglasnost_prekovremeni_od",
                "saglasnost_preraspodela_od",
            ] {
                assert!(
                    column_exists(&conn, "users", column),
                    "users.{column} should exist after v17"
                );
            }

            // The fifteen statutory buckets, letter-by-letter from ZEOR čl. 24 tač. 1.
            for column in [
                "moguci_minuta",
                "ukupno_ostvareni_minuta",
                "efektivno_izvrseni_minuta",
                "casovi_cekanja_i_zastoja_minuta",
                "obustava_rada_strajk_minuta",
                "ukupno_neizvrseni_minuta",
                "godisnji_odmor_minuta",
                "praznik_odmor_minuta",
                "odsustvo_uz_naknadu_minuta",
                "strucno_osposobljavanje_minuta",
                "sprecenost_poslodavac_minuta",
                "naknada_drugi_poslodavci_minuta",
                "sprecenost_rfzo_minuta",
                "porodiljsko_minuta",
                "neplaceno_odsustvo_minuta",
                "prekovremeni_minuta",
            ] {
                assert!(
                    column_exists(&conn, "work_time_entries", column),
                    "work_time_entries.{column} should exist after v17"
                );
            }

            // Advisory, not statutory — must exist but is labelled in code.
            assert!(column_exists(&conn, "work_time_entries", "nocni_minuta"));
            assert!(column_exists(
                &conn,
                "work_time_entries",
                "rad_na_praznik_minuta"
            ));

            // One row per (employee, date).
            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (700, 'radnik7', 'Radnik Sedam', 'cashier', 1, '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z'),
                            (701, 'radnik8', 'Radnik Osam', 'cashier', 1, '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                 INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                     VALUES (700, '2026-08-03', 480, '2026-08-03T18:00:00Z', '2026-08-03T18:00:00Z');",
            )
            .expect("first entry should insert");

            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                     VALUES (700, '2026-08-03', 60, '2026-08-03T19:00:00Z', '2026-08-03T19:00:00Z')",
                    [],
                )
                .is_err(),
                "a second row for the same (employee, day) must be rejected"
            );

            // Absence category is a closed enum enforced by the DB, not the UI.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, created_at, updated_at)
                     VALUES (700, '2026-08-04', 'nesto_izmisljeno', '2026-08-04T18:00:00Z', '2026-08-04T18:00:00Z')",
                    [],
                )
                .is_err(),
                "an unknown absence category must be rejected by the CHECK"
            );

            // §4 req. 3 / §5 item 4 — ZERO free text on an absence row. The correction
            // reason is a closed enum, so a doznaka number and a diagnosis are
            // structurally unrepresentable, not merely discouraged by review.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, korekcija_razlog, supersedes_id, created_at, updated_at)
                     VALUES (700, '2026-08-05', 'sprecenost_rfzo', 'ispravka — doznaka 1234/26, upala pluća', NULL, '2026-08-05T18:00:00Z', '2026-08-05T18:00:00Z')",
                    [],
                )
                .is_err(),
                "a doznaka number and a diagnosis must be rejected by the korekcija_razlog enum"
            );

            // A čl. 53 cap override is only meaningful on a worked day — it must never
            // ride on an absence row, and it is itself a closed enum.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, cap_override_razlog, created_at, updated_at)
                     VALUES (700, '2026-08-05', 'sprecenost_rfzo', 'visa_sila', '2026-08-05T18:00:00Z', '2026-08-05T18:00:00Z')",
                    [],
                )
                .is_err(),
                "a cap-override reason must not be recordable on an absence row"
            );
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, cap_override_razlog, created_at, updated_at)
                     VALUES (700, '2026-08-06', 'gazda je tako rekao', '2026-08-06T18:00:00Z', '2026-08-06T18:00:00Z')",
                    [],
                )
                .is_err(),
                "free text on cap_override_razlog must be rejected by the CHECK"
            );

            // `dan` keys both the uniqueness rule (§4 req. 1) and the čl. 53 weekly
            // bucket. '2026-8-3' would open a second slot for the same calendar date
            // and silently drop those minutes out of the weekly overtime total.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                     VALUES (700, '2026-8-3', 60, '2026-08-03T19:00:00Z', '2026-08-03T19:00:00Z')",
                    [],
                )
                .is_err(),
                "a non-ISO date must be rejected so it cannot bypass the unique index"
            );

            let original_id: i64 = conn
                .query_row(
                    "SELECT id FROM work_time_entries WHERE user_id = 700 AND dan = '2026-08-03'",
                    [],
                    |row| row.get(0),
                )
                .expect("the original row should exist");

            // §4 req. 6 + ZEOR čl. 46 st. 1 — a correction on an append-only statutory
            // record must carry who and why; the DB, not one command, enforces it.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, verzija, supersedes_id, created_at, updated_at)
                     VALUES (700, '2026-08-03', 2, ?1, '2026-08-04T09:00:00Z', '2026-08-04T09:00:00Z')",
                    [original_id],
                )
                .is_err(),
                "an anonymous, reasonless correction must be rejected"
            );

            // A correction must stay on its predecessor's (employee, day).
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, verzija, supersedes_id, korekcija_razlog, unio_user_id, created_at, updated_at)
                     VALUES (701, '2026-08-03', 2, ?1, 'greska_u_unosu', 700, '2026-08-04T09:00:00Z', '2026-08-04T09:00:00Z')",
                    [original_id],
                )
                .is_err(),
                "a correction must not chain to another employee's row"
            );

            // A chain root cannot masquerade as a correction, and vice versa.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, verzija, created_at, updated_at)
                     VALUES (700, '2026-08-07', 2, '2026-08-07T18:00:00Z', '2026-08-07T18:00:00Z')",
                    [],
                )
                .is_err(),
                "a row with no predecessor must be verzija 1"
            );

            conn.execute(
                "INSERT INTO work_time_entries (user_id, dan, verzija, efektivno_izvrseni_minuta, supersedes_id, korekcija_razlog, unio_user_id, created_at, updated_at)
                 VALUES (700, '2026-08-03', 2, 420, ?1, 'ispravka_sati', 700, '2026-08-04T09:00:00Z', '2026-08-04T09:00:00Z')",
                [original_id],
            )
            .expect("a well-formed correction should insert");

            // Forks are impossible: a second correction at the same verzija collides on
            // the unique index, and one that skips a verzija is rejected by the trigger.
            // Without this, two live rows could report 600 and 0 overtime minutes for the
            // same day and the register would be ambiguous about the čl. 53 st. 2 fact.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, verzija, prekovremeni_minuta, supersedes_id, korekcija_razlog, unio_user_id, created_at, updated_at)
                     VALUES (700, '2026-08-03', 2, 600, ?1, 'ispravka_sati', 700, '2026-08-04T10:00:00Z', '2026-08-04T10:00:00Z')",
                    [original_id],
                )
                .is_err(),
                "two corrections superseding the same predecessor must be rejected"
            );
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, verzija, prekovremeni_minuta, supersedes_id, korekcija_razlog, unio_user_id, created_at, updated_at)
                     VALUES (700, '2026-08-03', 3, 600, ?1, 'ispravka_sati', 700, '2026-08-04T10:00:00Z', '2026-08-04T10:00:00Z')",
                    [original_id],
                )
                .is_err(),
                "a correction must continue the previous verzija, not fork off the root"
            );

            // The live row is MAX(verzija) — and there is exactly one of it.
            let (live_rows, live_verzija, live_minuta): (i64, i64, i64) = conn
                .query_row(
                    "SELECT COUNT(*), MAX(verzija), MAX(efektivno_izvrseni_minuta)
                       FROM work_time_entries
                      WHERE user_id = 700 AND dan = '2026-08-03'
                        AND verzija = (SELECT MAX(verzija) FROM work_time_entries
                                        WHERE user_id = 700 AND dan = '2026-08-03')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("the live row should resolve");
            assert_eq!(
                live_rows, 1,
                "exactly one live row per (employee, day) after a correction"
            );
            assert_eq!(live_verzija, 2);
            assert_eq!(live_minuta, 420);

            // Append-only: the superseded original is still there, untouched.
            let originals: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM work_time_entries
                      WHERE user_id = 700 AND dan = '2026-08-03' AND efektivno_izvrseni_minuta = 480",
                    [],
                    |row| row.get(0),
                )
                .expect("the original should survive");
            assert_eq!(originals, 1, "a correction appends, it never mutates");

            conn.execute_batch(
                "INSERT INTO work_time_periods (user_id, godina, mesec, status, created_at, updated_at)
                     VALUES (700, 2026, 8, 'open', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');
                 INSERT INTO retention_policies (record_class, retain_until, legal_hold, created_at, updated_at)
                     VALUES ('worktime_classification', NULL, 0, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');",
            )
            .expect("period and retention rows should insert");
        }
        remove_test_database(&path);
    }

    /// Every `kategorija_odsustva` value must name its own minute bucket: the column
    /// is the enum value plus a `_minuta` suffix, with nothing else in between. The
    /// commands (Task 5) and the grid (Task 7) book an absence into a bucket; if the
    /// two vocabularies are allowed to drift, that mapping becomes a hand-maintained
    /// lookup table and one wrong line silently books ZEOR čl. 24 tač. 1 d) hours
    /// into the g) bucket, where nothing downstream can notice.
    #[test]
    fn migration_v17_derives_every_absence_bucket_from_its_category() {
        let path = test_database_path("migration_v17_category_bucket");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            let schema: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'work_time_entries'",
                    [],
                    |row| row.get(0),
                )
                .expect("the work_time_entries schema should be readable");

            let categories = closed_enum_values(&schema, "kategorija_odsustva");
            assert_eq!(
                categories.len(),
                10,
                "kategorija_odsustva must stay a closed enum of ten values; parsed {categories:?}"
            );

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (710, 'radnik10', 'Radnik Deset', 'cashier', 1, '2026-09-01T08:00:00Z', '2026-09-01T08:00:00Z');",
            )
            .expect("the seed employee should insert");

            for (offset, category) in categories.iter().enumerate() {
                let bucket = format!("{category}_minuta");
                assert!(
                    column_exists(&conn, "work_time_entries", &bucket),
                    "kategorija_odsustva = {category} must book into {bucket}"
                );

                // The CHECK really admits the value we parsed — the derivation is over
                // the live constraint, not over a comment that drifted away from it.
                let dan = format!("2026-09-{:02}", offset + 1);
                conn.execute(
                    &format!(
                        "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, {bucket}, created_at, updated_at)
                         VALUES (710, ?1, ?2, 480, '2026-09-01T18:00:00Z', '2026-09-01T18:00:00Z')"
                    ),
                    rusqlite::params![dan, category],
                )
                .unwrap_or_else(|error| {
                    panic!("{category} should be insertable into {bucket}: {error}")
                });
            }

            // The suffix rule is the whole contract, so a category that does not carry
            // it must not be smuggled into the enum by a later edit.
            assert!(
                conn.execute(
                    "INSERT INTO work_time_entries (user_id, dan, kategorija_odsustva, created_at, updated_at)
                     VALUES (710, '2026-09-20', 'placeno_odsustvo', '2026-09-20T18:00:00Z', '2026-09-20T18:00:00Z')",
                    [],
                )
                .is_err(),
                "a category with no bucket column of the same name must be rejected"
            );
        }
        remove_test_database(&path);
    }

    /// **The `CHECK` and [`crate::commands::worktime::KATEGORIJE_ODSUSTVA`] are one
    /// vocabulary, and until this test nothing said so.** v17 is the storage gate —
    /// SQLite refuses an eleventh category outright; the constant is the validation
    /// gate, so `book_absence` turns that refusal into a clean error instead of a raw
    /// `CHECK` violation, and no operator string ever reaches SQL unvalidated. Two
    /// declarations of the ZEOR čl. 24 tač. 1 buckets, in two languages, with no
    /// compiler able to see that they must agree.
    ///
    /// Neither existing test closes the gap, because each reads exactly one side.
    /// `migration_v17_derives_every_absence_bucket_from_its_category` parses the
    /// schema and never looks at the constant. `commands::audit`'s
    /// `the_unmask_line_carries_no_category_no_name_and_no_month` iterates the
    /// constant and never looks at the schema. So an eleventh category added to the
    /// schema alone leaves both green while the req. 28 leak test silently stops
    /// covering it — and that test is the one holding the čl. 48 unmask line to
    /// carrying no čl. 17 posebna vrsta value. Drift there is not a stale list; it
    /// is a category that may be written into `audit_events` with every test
    /// passing. Drift the other way is milder but still wrong: a category the
    /// constant admits and the schema rejects turns a validation error into a
    /// `CHECK` violation surfacing from the driver.
    ///
    /// Count **and** set, and deliberately not order. Order inside a `CHECK … IN`
    /// list carries no meaning, so pinning it would fail on a harmless reshuffle;
    /// but set equality alone would swallow a literal duplicated on one side, which
    /// is exactly how a hand-edited ten-line list drifts. Both assertions together
    /// fail in either direction.
    #[test]
    fn migration_v17_check_and_kategorije_odsustva_are_the_same_ten_values() {
        use std::collections::BTreeSet;

        let path = test_database_path("migration_v17_kategorije_pin");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            let schema: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'work_time_entries'",
                    [],
                    |row| row.get(0),
                )
                .expect("the work_time_entries schema should be readable");

            let in_schema = closed_enum_values(&schema, "kategorija_odsustva");
            let in_code = crate::commands::worktime::KATEGORIJE_ODSUSTVA;

            // Counted over the raw lists, before the sets collapse any duplicate.
            assert_eq!(
                in_schema.len(),
                in_code.len(),
                "v17 admits {} kategorija_odsustva values and KATEGORIJE_ODSUSTVA lists {}; \
                 schema was {in_schema:?}, constant was {in_code:?}",
                in_schema.len(),
                in_code.len()
            );

            let schema_set: BTreeSet<&str> = in_schema.iter().map(String::as_str).collect();
            let code_set: BTreeSet<&str> = in_code.iter().copied().collect();
            let only_in_schema: Vec<&str> = schema_set.difference(&code_set).copied().collect();
            let only_in_code: Vec<&str> = code_set.difference(&schema_set).copied().collect();
            assert_eq!(
                schema_set, code_set,
                "the v17 CHECK and KATEGORIJE_ODSUSTVA must name the same absence \
                 categories. Only in the schema: {only_in_schema:?} — book_absence would \
                 reject these before SQLite ever saw them. Only in the constant: \
                 {only_in_code:?} — these reach SQLite and come back as a raw CHECK \
                 violation. Either way the req. 28 unmask-leak test in commands::audit \
                 iterates the constant, so a category missing from it is a category that \
                 test no longer proves absent from the čl. 48 line."
            );
        }
        remove_test_database(&path);
    }

    #[test]
    fn migration_v17_preserves_pre_existing_users() {
        let path = test_database_path("migration_v17_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..16] {
                assert!(
                    migration.version <= 16,
                    "the pre-v17 prefix must stop at v16, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-07-01T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, pin_hash, active, created_at, updated_at, last_login_at)
                 VALUES (701, 'stara', 'Stara Radnica', 'cashier', 'hash-701', 1,
                         '2025-01-02T08:00:00Z', '2025-06-02T08:00:00Z', '2026-07-30T08:00:00Z');",
            )
            .expect("seed a v16-era user");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (username, pin, created, last): (String, String, String, String) = conn
                .query_row(
                    "SELECT username, pin_hash, created_at, last_login_at FROM users WHERE id = 701",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the pre-v17 user must survive verbatim");

            assert_eq!(username, "stara");
            assert_eq!(pin, "hash-701");
            assert_eq!(created, "2025-01-02T08:00:00Z");
            assert_eq!(last, "2026-07-30T08:00:00Z");

            let profile: Option<String> = conn
                .query_row(
                    "SELECT datum_rodjenja FROM users WHERE id = 701",
                    [],
                    |row| row.get(0),
                )
                .expect("the new column exists");
            assert_eq!(
                profile, None,
                "new profile columns are nullable, not backfilled"
            );
        }
        remove_test_database(&path);
    }

    #[test]
    fn migration_v18_adds_the_zzpl_stores() {
        let path = test_database_path("migration_v18_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for table in [
                "audit_events",
                "support_sessions",
                "personnel_records",
                "data_breaches",
                "processing_activities",
            ] {
                assert!(table_exists(&conn, table), "{table} should exist after v18");
            }

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                 VALUES (800, 'revizor', 'Revizor Osam', 'admin', 1, '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');",
            )
            .expect("seed an actor");

            // The action verb is a closed enum mirroring ZZPL čl. 48 st. 1.
            assert!(
                conn.execute(
                    "INSERT INTO audit_events (at, actor_user_id, action, object_type, object_id, prev_hash, hash)
                     VALUES ('2026-08-01T09:00:00Z', 800, 'izmisljeno', 'sale', '1', '', 'h1')",
                    [],
                )
                .is_err(),
                "an action outside čl. 48 st. 1 must be rejected by the CHECK"
            );

            conn.execute(
                "INSERT INTO audit_events (at, actor_user_id, action, object_type, object_id, reason_code, prev_hash, hash)
                 VALUES ('2026-08-01T09:00:00Z', 800, 'uvid', 'employee', '3', 'inspekcija', '', 'h1')",
                [],
            )
            .expect("a well-formed uvid should insert");

            // Req. 4: the audit log holds no personal data, and a search query for a
            // customer's name IS personal data about that customer. object_type is a
            // code constant ('sale', 'employee'), so it carries the same no-space
            // shape as object_id — otherwise 'pretraga:Marko Marković' fits in it.
            assert!(
                conn.execute(
                    "INSERT INTO audit_events (at, actor_user_id, action, object_type, object_id, reason_code, prev_hash, hash)
                     VALUES ('2026-08-01T09:05:00Z', 800, 'uvid', 'pretraga:Marko Markovic', '3', 'inspekcija', '', 'h2')",
                    [],
                )
                .is_err(),
                "an object_type carrying spaces must be rejected by the CHECK"
            );

            // ZZPL čl. 48 st. 2 requires the razlog for uvid and otkrivanje, and it is
            // a constraint rather than a convention.
            assert!(
                conn.execute(
                    "INSERT INTO audit_events (at, actor_user_id, action, object_type, object_id, prev_hash, hash)
                     VALUES ('2026-08-01T09:10:00Z', 800, 'uvid', 'employee', '3', '', 'h3')",
                    [],
                )
                .is_err(),
                "an uvid without a reason_code must be rejected by the CHECK"
            );

            // Čl. 48 st. 2 also requires the identitet primaoca for a disclosure; a
            // razlog alone does not answer the question the article asks.
            assert!(
                conn.execute(
                    "INSERT INTO audit_events (at, actor_user_id, action, object_type, object_id, reason_code, prev_hash, hash)
                     VALUES ('2026-08-01T09:15:00Z', 800, 'otkrivanje', 'employee', '3', 'zakonska_obaveza', '', 'h4')",
                    [],
                )
                .is_err(),
                "an otkrivanje without a recipient must be rejected by the CHECK"
            );

            // ZZPL čl. 52 st. 1 anchors the 72 h clock to saznanje, so it cannot be null.
            assert!(
                conn.execute(
                    "INSERT INTO data_breaches (opis, posledice, mere, created_at, updated_at)
                     VALUES ('x', 'y', 'z', '2026-08-01T09:00:00Z', '2026-08-01T09:00:00Z')",
                    [],
                )
                .is_err(),
                "a breach without saznanje_at must be rejected"
            );
        }
        remove_test_database(&path);
    }

    #[test]
    fn migration_v18_preserves_pre_existing_users_and_shields_personnel() {
        let path = test_database_path("migration_v18_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..17] {
                assert!(
                    migration.version <= 17,
                    "the pre-v18 prefix must stop at v17, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-07-01T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, pin_hash, active, created_at, updated_at)
                 VALUES (801, 'stari', 'Stari Radnik', 'cashier', 'hash-801', 1,
                         '2025-02-03T08:00:00Z', '2025-09-03T08:00:00Z');",
            )
            .expect("seed a v17-era user");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (username, pin): (String, String) = conn
                .query_row(
                    "SELECT username, pin_hash FROM users WHERE id = 801",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the pre-v18 user must survive verbatim");
            assert_eq!(username, "stari");
            assert_eq!(pin, "hash-801");

            // ZZPL req. 24: deleting an account must NOT be able to take the
            // personnel record with it. Prove there is no cascade.
            conn.execute(
                "INSERT INTO personnel_records (user_id, prezime_ime, created_at, updated_at)
                 VALUES (801, 'Stari Radnik', '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z')",
                [],
            )
            .expect("personnel record inserts");

            assert!(
                conn.execute("DELETE FROM users WHERE id = 801", [])
                    .is_err(),
                "the personnel FK must RESTRICT the account delete, not cascade it away"
            );
        }
        remove_test_database(&path);
    }

    /// The two structural guarantees v18 exists to make unbypassable.
    ///
    /// **Append-only** (req. 7): an owner-editable audit log proves nothing, and
    /// proving something is the entire reason it exists — so `audit_events` carries
    /// no `updated_at` and every UPDATE aborts in the engine, not in a command.
    /// DELETE stays open on purpose: req. 6 forbids `trajno` on this log, so
    /// expiry must be able to remove a row, and the hash chain is what detects the
    /// resulting gap (proved in `audit::tests`).
    ///
    /// **Trajno** (req. 19, 24): the ZEOR čl. 5 personnel record is class A, which
    /// no purge, reset or UI action may reach. A trigger, not a convention.
    #[test]
    fn migration_v18_makes_the_audit_log_append_only_and_the_personnel_record_trajno() {
        let path = test_database_path("migration_v18_append_only");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            assert!(
                !column_exists(&conn, "audit_events", "updated_at"),
                "an audit row is never updated, so it must not carry an updated_at"
            );

            let personnel_schema: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'personnel_records'",
                    [],
                    |row| row.get(0),
                )
                .expect("the personnel_records schema should be readable");
            assert!(
                !personnel_schema.contains("ON DELETE CASCADE"),
                "no ON DELETE CASCADE may point at the personnel record, schema was: {personnel_schema}"
            );

            conn.execute_batch(
                "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (810, 'vlasnik', 'Vlasnik Deset', 'admin', 1, '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                 INSERT INTO audit_events (id, at, actor_user_id, action, object_type, object_id, prev_hash, hash)
                     VALUES (900, '2026-08-01T09:00:00Z', 810, 'unos', 'sale', '7', '', 'h-900');
                 INSERT INTO personnel_records (user_id, prezime_ime, created_at, updated_at)
                     VALUES (810, 'Vlasnik Deset', '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                 INSERT INTO data_breaches (saznanje_at, opis, posledice, mere, created_at, updated_at)
                     VALUES ('2026-08-01T09:00:00Z', 'opis', 'posledice', 'mere', '2026-08-01T09:00:00Z', '2026-08-01T09:00:00Z');",
            )
            .expect("the seed rows should insert");

            assert!(
                conn.execute(
                    "UPDATE audit_events SET object_id = '999' WHERE id = 900",
                    [],
                )
                .is_err(),
                "no path may update an audit row"
            );

            assert!(
                conn.execute("DELETE FROM personnel_records WHERE user_id = 810", [])
                    .is_err(),
                "the personnel record is trajno — no path may delete it"
            );

            // ZZPL čl. 52 st. 1: the 72 h clock is anchored to saznanje, so the
            // anchor cannot be moved after the fact.
            assert!(
                conn.execute(
                    "UPDATE data_breaches SET saznanje_at = '2026-08-05T09:00:00Z' WHERE id = 1",
                    [],
                )
                .is_err(),
                "saznanje_at must be immutable once set"
            );
            conn.execute(
                "UPDATE data_breaches SET mere = 'dopunjene mere' WHERE id = 1",
                [],
            )
            .expect("the rest of the breach record stays editable");

            // Storage limitation (req. 6) must stay reachable: this log is never trajno.
            conn.execute("DELETE FROM audit_events WHERE id = 900", [])
                .expect("an expired audit row must remain deletable");
        }
        remove_test_database(&path);
    }

    /// SW-12 req. 10 and 14. The published cenovnik is an archive, not a cache:
    /// ZZP čl. 6 st. 5 obliges the trader to enable comparison of prethodno
    /// objavljene cene against cene objavljene u realnom vremenu, which is only
    /// possible if every publication survives the next one. And čl. 6 st. 2's
    /// second sentence pulls st. 1 into the file, so the catalog has to be able to
    /// express a jedinična cena — not only a prodajna cena.
    #[test]
    fn migration_v19_adds_the_cenovnik_archive_and_the_unit_price_fields() {
        let path = test_database_path("migration_v19_schema");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            assert!(
                table_exists(&conn, "cenovnik_snapshots"),
                "cenovnik_snapshots should exist after v19"
            );

            for column in [
                "id",
                "prodajno_mesto",
                "generated_at",
                "row_count",
                "content_hash",
                "body",
                "published_at",
                "published_target",
                "created_at",
            ] {
                assert!(
                    column_exists(&conn, "cenovnik_snapshots", column),
                    "cenovnik_snapshots should carry {column}"
                );
            }

            // Req. 10: the unit price and the measure it is expressed in. Both are
            // nullable — a product sold and priced per piece legitimately has none.
            for column in ["jedinicna_cena_jedinica", "jedinicna_cena_sadrzaj_milli"] {
                assert!(
                    column_exists(&conn, "products", column),
                    "products should carry {column}"
                );
            }

            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (190, 'PDV 20', 2000, '2026-08-02T08:00:00Z', '2026-08-02T08:00:00Z');
                 INSERT INTO products (id, name, sku, unit_of_measure, sale_price_minor, tax_rate_id,
                                       created_at, updated_at)
                     VALUES (190, 'Sok od jabuke 0,75 l', 'SOK-075', 'kom', 27900, 190,
                             '2026-08-02T08:00:00Z', '2026-08-02T08:00:00Z');",
            )
            .expect("seed a product");

            // The sadržaj is on the schema-wide milli scale — value × 1000, the same
            // divisor `quantity_milli` and `minimum_stock_milli` use — so a 0,75 l
            // bottle is 750, not 750000. A column reading `_milli` but meaning
            // something else than everywhere else in the same database would publish a
            // jedinična cena wrong by three orders of magnitude, which is precisely
            // the čl. 6 st. 2 defect req. 10 exists to prevent.
            conn.execute(
                "UPDATE products SET jedinicna_cena_jedinica = 'l',
                                     jedinicna_cena_sadrzaj_milli = 750
                 WHERE id = 190",
                [],
            )
            .expect("a measure with its content should store");

            let (jedinica, jedinicna_cena_minor): (String, i64) = conn
                .query_row(
                    "SELECT jedinicna_cena_jedinica,
                            sale_price_minor * 1000 / jedinicna_cena_sadrzaj_milli
                       FROM products WHERE id = 190",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the jedinična cena should derive from the stored sadržaj");
            assert_eq!(jedinica, "l");
            assert_eq!(
                jedinicna_cena_minor, 37_200,
                "27.900 para for a 0,75 l bottle is 372,00 RSD/l — a sadržaj stored on \
                 any scale but the schema-wide milli is off by a factor of 1000"
            );

            // A sadržaj without the measure it is expressed in cannot produce a
            // jedinična cena, so the schema refuses to hold that half-state.
            assert!(
                conn.execute(
                    "UPDATE products SET jedinicna_cena_jedinica = NULL WHERE id = 190",
                    [],
                )
                .is_err(),
                "a content without its measure must be rejected"
            );
            assert!(
                conn.execute(
                    "UPDATE products SET jedinicna_cena_sadrzaj_milli = 0 WHERE id = 190",
                    [],
                )
                .is_err(),
                "a zero content would divide the prodajna cena by nothing"
            );

            // Čl. 6 st. 2 „posebno za svaki prodajni objekat“: the outlet is part of
            // the archive's identity, so a blank one is not a snapshot of anything.
            assert!(
                conn.execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('', '2026-08-02T09:00:00Z', 1, 'h1', 'telo', '2026-08-02T09:00:00Z')",
                    [],
                )
                .is_err(),
                "a snapshot without a prodajno mesto must be rejected"
            );
            assert!(
                conn.execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Radnja 1', '2026-08-02T09:00:00Z', 1, '', 'telo', '2026-08-02T09:00:00Z')",
                    [],
                )
                .is_err(),
                "a snapshot without a content hash cannot be compared against anything"
            );

            // An undated row sorts below every RFC3339 stamp, so it could never be
            // selected as an outlet's current cenovnik and could never take part in
            // the čl. 6 st. 5 comparison — a snapshot the archive cannot date.
            assert!(
                conn.execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Radnja 1', '', 1, 'h1', 'telo', '2026-08-02T09:00:00Z')",
                    [],
                )
                .is_err(),
                "a snapshot the archive cannot date must be rejected"
            );
            assert!(
                conn.execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Radnja 1', '2026-08-02T09:00:00Z', 1, 'h1', 'telo', '')",
                    [],
                )
                .is_err(),
                "a snapshot without a created_at must be rejected"
            );

            conn.execute_batch(
                "INSERT INTO cenovnik_snapshots (id, prodajno_mesto, generated_at, row_count,
                                                 content_hash, body, created_at)
                     VALUES (1, 'Radnja 1', '2026-08-02T09:00:00Z', 2, 'h-stari', 'stari cenovnik',
                             '2026-08-02T09:00:00Z'),
                            (2, 'Radnja 1', '2026-08-02T11:00:00Z', 2, 'h-novi', 'novi cenovnik',
                             '2026-08-02T11:00:00Z'),
                            (3, 'Radnja 2', '2026-08-02T10:00:00Z', 1, 'h-druga', 'druga radnja',
                             '2026-08-02T10:00:00Z'),
                            (4, 'Radnja 1', '2026-08-02T11:00:00Z', 3, 'h-isti-tren',
                             'jos noviji cenovnik', '2026-08-02T11:00:00Z');",
            )
            .expect("four snapshots should insert");

            // No stored current-snapshot pointer exists to go stale: „the current
            // cenovnik for an outlet“ is derived as the newest row for that outlet,
            // ties on generated_at broken by the larger id. Rows 2 and 4 share a
            // generated_at to the second — two price saves inside one second is
            // ordinary at a till — so here the tie-break alone decides which file
            // čl. 6 st. 4 binds the shop to, and it must be the later insert.
            let current_for = |outlet: &str| -> i64 {
                conn.query_row(
                    "SELECT id FROM cenovnik_snapshots
                      WHERE prodajno_mesto = ?1
                      ORDER BY generated_at DESC, id DESC
                      LIMIT 1",
                    rusqlite::params![outlet],
                    |row| row.get(0),
                )
                .expect("the current snapshot should resolve")
            };
            assert_eq!(
                current_for("Radnja 1"),
                4,
                "on a same-second tie the larger id is the current cenovnik"
            );
            assert_eq!(
                current_for("Radnja 2"),
                3,
                "each outlet resolves its own current cenovnik"
            );

            let kept: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM cenovnik_snapshots WHERE prodajno_mesto = 'Radnja 1'",
                    [],
                    |row| row.get(0),
                )
                .expect("the archive should count");
            assert_eq!(
                kept, 3,
                "čl. 6 st. 5: a publication is never overwritten by the next one"
            );
        }
        remove_test_database(&path);
    }

    /// A published cenovnik is evidence of what the shop offered at a moment in
    /// time (čl. 6 st. 4 binds the trader to the prices it published), so no UPDATE
    /// may rewrite its body, its hash or its identity. That is the whole of what
    /// the engine enforces, and this test pins both halves of the boundary: DELETE
    /// stays open, because the čl. 213 two-year limitation gives the archive a
    /// retention floor rather than a trajno duty and the purge (req. 14) must be
    /// able to reach an expired row — and INSERT OR REPLACE, which SQLite runs as a
    /// DELETE followed by an INSERT without firing an UPDATE trigger, is therefore
    /// open too. Closing that in the engine would close the purge with it, so it is
    /// a constraint on the write path instead, asserted here so the schema comment
    /// can never quietly become a promise the engine does not keep.
    #[test]
    fn migration_v19_blocks_every_update_but_leaves_delete_and_replace_open() {
        let path = test_database_path("migration_v19_immutable");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            assert!(
                !column_exists(&conn, "cenovnik_snapshots", "updated_at"),
                "a snapshot is never edited, so it must not carry an updated_at"
            );

            conn.execute(
                "INSERT INTO cenovnik_snapshots (id, prodajno_mesto, generated_at, row_count,
                                                 content_hash, body, created_at)
                 VALUES (10, 'Radnja 1', '2026-08-02T09:00:00Z', 2, 'h-10', 'telo cenovnika',
                         '2026-08-02T09:00:00Z')",
                [],
            )
            .expect("a snapshot should insert");

            // `id` belongs in this list: it is the archive's only handle on a
            // snapshot — the tie-break that decides which row is an outlet's current
            // cenovnik, and the only thing a Task 5 divergence can name as the
            // snapshot it was compared against. A mutable primary key would let that
            // evidence link be silently repointed at a different published file.
            for (column, value) in [
                ("body", "izmenjeno telo"),
                ("content_hash", "h-lazni"),
                ("generated_at", "2026-08-02T12:00:00Z"),
                ("prodajno_mesto", "Radnja 2"),
                ("row_count", "9"),
                ("created_at", "2026-08-02T12:00:00Z"),
                ("id", "999"),
            ] {
                assert!(
                    conn.execute(
                        &format!(
                            "UPDATE cenovnik_snapshots SET {column} = '{value}' WHERE id = 10"
                        ),
                        [],
                    )
                    .is_err(),
                    "no path may rewrite {column} on a snapshot"
                );
            }

            // The publication stamp is the one thing written after the fact: a
            // snapshot is generated first and only then accepted by a target.
            conn.execute(
                "UPDATE cenovnik_snapshots
                    SET published_at = '2026-08-02T09:00:05Z', published_target = 'local_folder'
                  WHERE id = 10",
                [],
            )
            .expect("an unpublished snapshot should accept its publication stamp");

            assert!(
                conn.execute(
                    "UPDATE cenovnik_snapshots
                        SET published_at = '2026-08-03T09:00:00Z', published_target = 'drugo'
                      WHERE id = 10",
                    [],
                )
                .is_err(),
                "a publication already recorded must not be restamped"
            );

            let (published_at, target): (String, String) = conn
                .query_row(
                    "SELECT published_at, published_target FROM cenovnik_snapshots WHERE id = 10",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the publication stamp should read back");
            assert_eq!(published_at, "2026-08-02T09:00:05Z");
            assert_eq!(target, "local_folder");

            // The gap the engine cannot close without closing the purge. REPLACE is a
            // DELETE plus an INSERT: no UPDATE happens for either trigger to see, and
            // with recursive_triggers off (the default this app runs on — db/mod.rs
            // sets foreign_keys, busy_timeout and journal_mode and nothing else) the
            // delete half fires no trigger either. So this succeeds, and it silently
            // rewrites a published body and drops its publication stamp. It is
            // closed by discipline on the write path, not by
            // the schema: the publish path uses a plain INSERT only — never INSERT OR
            // REPLACE, never ON CONFLICT DO UPDATE — and the retention purge is the
            // only code permitted to DELETE from this table.
            conn.execute(
                "INSERT OR REPLACE INTO cenovnik_snapshots (id, prodajno_mesto, generated_at,
                                                            row_count, content_hash, body,
                                                            created_at)
                 VALUES (10, 'Radnja 1', '2026-08-02T09:00:00Z', 2, 'h-10', 'podmetnuto telo',
                         '2026-08-02T09:00:00Z')",
                [],
            )
            .expect("REPLACE is a DELETE plus an INSERT — no UPDATE trigger sees it");

            let (body, still_published): (String, Option<String>) = conn
                .query_row(
                    "SELECT body, published_at FROM cenovnik_snapshots WHERE id = 10",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the replaced row should read back");
            assert_eq!(
                body, "podmetnuto telo",
                "the engine does NOT stop REPLACE — the publish path must never use it"
            );
            assert!(
                still_published.is_none(),
                "REPLACE also drops the publication stamp, leaving no trace of the overwrite"
            );

            conn.execute("DELETE FROM cenovnik_snapshots WHERE id = 10", [])
                .expect("an expired snapshot must remain purgeable");
        }
        remove_test_database(&path);
    }

    /// The installed-base path: a till already running v18 gets the archive and the
    /// unit-price columns without losing a product, a price or a price_history row.
    #[test]
    fn migration_v19_preserves_pre_existing_products_and_price_history() {
        let path = test_database_path("migration_v19_survival");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw connection");
            conn.execute_batch(
                "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
            )
            .expect("migrations table");

            for migration in &MIGRATIONS[..18] {
                assert!(
                    migration.version <= 18,
                    "the pre-v19 prefix must stop at v18, saw v{}",
                    migration.version
                );
                conn.execute_batch(migration.sql)
                    .unwrap_or_else(|error| panic!("v{} should apply: {error}", migration.version));
                conn.execute(
                    "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, '2026-08-01T00:00:00Z')",
                    rusqlite::params![migration.version, migration.name],
                )
                .expect("record the migration");
            }

            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (191, 'PDV 20', 2000, '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO products (id, name, sku, barcode, unit_of_measure, sale_price_minor,
                                       purchase_price_minor, tax_rate_id, minimum_stock_milli,
                                       created_at, updated_at)
                     VALUES (191, 'Marama svilena', 'MAR-001', '0123456789012', 'kom', 249900,
                             120000, 191, 0, '2026-07-01T08:00:00Z', '2026-07-20T08:00:00Z');
                 INSERT INTO price_history (id, product_id, effective_from, price_minor, source, created_at)
                     VALUES (77, 191, '2026-07-20T08:00:00Z', 249900, 'update', '2026-07-20T08:00:00Z');
                 INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (191, 'stari_kasir', 'Stari Kasir', 'cashier', 1,
                             '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z');
                 INSERT INTO compliance_log (id, event_type, detail_json, user_id, created_at)
                     VALUES (191, 'aml_cash_threshold', '{\"cash_minor\":1000000}', 191,
                             '2026-07-25T11:00:00Z');",
            )
            .expect("seed v18-era catalog rows");
            drop(conn);

            let db = Db::new(&path).expect("database should migrate forward");
            let conn = db.open().expect("database should open");

            let (name, barcode, price, updated): (String, String, i64, String) = conn
                .query_row(
                    "SELECT name, barcode, sale_price_minor, updated_at FROM products WHERE id = 191",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the pre-v19 product must survive verbatim");
            assert_eq!(name, "Marama svilena");
            assert_eq!(
                barcode, "0123456789012",
                "the leading zero of a 13-digit barcode must survive as text"
            );
            assert_eq!(price, 249900);
            assert_eq!(updated, "2026-07-20T08:00:00Z");

            let (jedinica, sadrzaj): (Option<String>, Option<i64>) = conn
                .query_row(
                    "SELECT jedinicna_cena_jedinica, jedinicna_cena_sadrzaj_milli
                     FROM products WHERE id = 191",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the new unit-price columns exist");
            assert_eq!(
                (jedinica, sadrzaj),
                (None, None),
                "the unit-price fields are nullable and deliberately not backfilled — \
                 a guessed jedinična cena would be published as fact"
            );

            let (history_product, history_price): (i64, i64) = conn
                .query_row(
                    "SELECT product_id, price_minor FROM price_history WHERE id = 77",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the price_history row v19 publishes off must survive with its id");
            assert_eq!(history_product, 191);
            assert_eq!(history_price, 249900);

            let snapshots: i64 = conn
                .query_row("SELECT COUNT(*) FROM cenovnik_snapshots", [], |row| {
                    row.get(0)
                })
                .expect("the archive should exist on an upgraded database");
            assert_eq!(
                snapshots, 0,
                "an upgrade publishes nothing on its own — that is the write path's job"
            );

            // v19 also rebuilds `compliance_log` to admit the SW-12 till-guard
            // event (req. 12). It is the never-deleted trail: an upgrade that lost
            // an AML entry would destroy the only evidence that the warning was
            // ever shown, so the copy step is the whole point of the rebuild.
            let (event_type, detail, user_id, created_at): (String, String, i64, String) = conn
                .query_row(
                    "SELECT event_type, detail_json, user_id, created_at
                     FROM compliance_log WHERE id = 191",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the pre-v19 audit row must survive with its id");
            assert_eq!(
                (
                    event_type.as_str(),
                    detail.as_str(),
                    user_id,
                    created_at.as_str()
                ),
                (
                    "aml_cash_threshold",
                    "{\"cash_minor\":1000000}",
                    191,
                    "2026-07-25T11:00:00Z"
                ),
                "the v19 rebuild must copy every audit row verbatim"
            );

            let index_exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                      WHERE type = 'index' AND name = 'idx_compliance_log_created_at'",
                    [],
                    |row| row.get(0),
                )
                .expect("index metadata should query");
            assert_eq!(
                index_exists, 1,
                "the rebuild drops the table, so the index the trail is read through must be back"
            );
        }
        remove_test_database(&path);
    }

    /// Req. 12. The divergence between a rung price and the published one is a
    /// compliance event of the same kind as the čl. 46 AML entry — warned about at
    /// the till, never refused, recorded in the one never-deleted trail — so v19
    /// widens the `event_type` CHECK to admit it. The vocabulary stays closed:
    /// a trail read back as evidence cannot accept an event type nobody defined.
    #[test]
    fn migration_v19_admits_the_cenovnik_divergence_event_and_nothing_else() {
        let path = test_database_path("migration_v19_divergence_event");
        {
            let db = Db::new(&path).expect("database should initialize");
            let conn = db.open().expect("database should open");

            for event_type in [
                "trading_data_reset",
                "backup_restored",
                "aml_cash_threshold",
                "cenovnik_price_divergence",
            ] {
                conn.execute(
                    "INSERT INTO compliance_log (event_type, detail_json, created_at)
                     VALUES (?1, '{}', '2026-08-02T09:00:00Z')",
                    params![event_type],
                )
                .unwrap_or_else(|error| panic!("{event_type} should be admitted: {error}"));
            }

            assert!(
                conn.execute(
                    "INSERT INTO compliance_log (event_type, detail_json, created_at)
                     VALUES ('nepoznat_dogadjaj', '{}', '2026-08-02T09:00:00Z')",
                    [],
                )
                .is_err(),
                "the trail's vocabulary is closed"
            );
        }
        remove_test_database(&path);
    }
}
