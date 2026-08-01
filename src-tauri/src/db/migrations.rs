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
        std::fs::remove_file(&path).expect("test database should be removed");
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
        std::fs::remove_file(&path).expect("test database should be removed");
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
        std::fs::remove_file(&path).expect("test database should be removed");
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
        std::fs::remove_file(&path).expect("test database should be removed");
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
        std::fs::remove_file(&path).expect("test database should be removed");
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
        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
