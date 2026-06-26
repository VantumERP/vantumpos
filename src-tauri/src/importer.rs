use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app_error::{AppError, CommandError};
use crate::db::Db;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportType {
    Products,
    Categories,
    InitialStock,
}

impl ImportType {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Products => "products",
            Self::Categories => "categories",
            Self::InitialStock => "initial_stock",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRowStatus {
    Valid,
    Warning,
    Error,
    Imported,
    Skipped,
}

impl ImportRowStatus {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Imported => "imported",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRowAction {
    Create,
    Update,
    Skip,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportJobStatus {
    Draft,
    Validated,
    Completed,
    Failed,
}

impl ImportJobStatus {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Validated => "validated",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadImportHeadersRequest {
    pub import_type: ImportType,
    pub file_name: String,
    pub csv_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateImportRequest {
    pub import_type: ImportType,
    pub file_name: String,
    pub csv_text: String,
    pub mapping: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitImportRequest {
    pub import_type: ImportType,
    pub file_name: String,
    pub csv_text: String,
    pub mapping: HashMap<String, String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportHeaders {
    pub import_type: ImportType,
    pub file_name: String,
    pub delimiter: String,
    pub headers: Vec<String>,
    pub total_rows: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub create: usize,
    pub update: usize,
    pub skip: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRowResult {
    pub row_number: usize,
    pub status: ImportRowStatus,
    pub action: ImportRowAction,
    pub message: String,
    pub values: HashMap<String, String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportValidationResult {
    pub import_type: ImportType,
    pub file_name: String,
    pub total_rows: usize,
    pub valid_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub summary: ImportSummary,
    pub rows: Vec<ImportRowResult>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobSummary {
    pub id: i64,
    pub import_type: ImportType,
    pub file_name: String,
    pub status: ImportJobStatus,
    pub total_rows: usize,
    pub error_rows: usize,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobDetail {
    pub id: i64,
    pub import_type: ImportType,
    pub file_name: String,
    pub status: ImportJobStatus,
    pub total_rows: usize,
    pub error_rows: usize,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub rows: Vec<ImportJobRowDetail>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobRowDetail {
    pub row_number: usize,
    pub status: ImportRowStatus,
    pub message: Option<String>,
    pub values: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ImportError {
    code: &'static str,
    message: String,
    details: Option<serde_json::Value>,
}

impl ImportError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    fn with_details(
        code: &'static str,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details),
        }
    }

    #[cfg(test)]
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl From<rusqlite::Error> for ImportError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("database_error", format!("Greska baze podataka: {error}"))
    }
}

impl From<AppError> for ImportError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Database(source) => source.into(),
            AppError::Io(source) => Self::new(
                "file_system_error",
                format!("Greska fajl sistema: {source}"),
            ),
            AppError::Validation { message, details } => Self {
                code: "validation_error",
                message,
                details,
            },
            AppError::NotFound(message) => Self::new("not_found", message),
            AppError::BackupFailed(message) => Self::new("backup_failed", message),
            AppError::Business {
                code,
                message,
                details,
            } => Self {
                code,
                message,
                details,
            },
            AppError::InvalidState(message) => Self::new("invalid_state", message),
        }
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(
            "serialization_error",
            format!("Greska pripreme podataka: {error}"),
        )
    }
}

impl From<time::error::Format> for ImportError {
    fn from(error: time::error::Format) -> Self {
        Self::new("time_error", format!("Greska vremena: {error}"))
    }
}

impl From<ImportError> for CommandError {
    fn from(error: ImportError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    }
}

struct ParsedCsv {
    delimiter: char,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

struct HeaderLookup {
    indexes: HashMap<String, usize>,
}

impl HeaderLookup {
    fn new(headers: &[String]) -> Self {
        Self {
            indexes: headers
                .iter()
                .enumerate()
                .map(|(index, header)| (header.to_string(), index))
                .collect(),
        }
    }

    fn value<'a>(
        &self,
        row: &'a [String],
        mapping: &HashMap<String, String>,
        field: &str,
    ) -> Option<&'a str> {
        let header = mapping.get(field)?.trim();

        if header.is_empty() {
            return None;
        }

        self.indexes
            .get(header)
            .and_then(|index| row.get(*index))
            .map(String::as_str)
    }
}

pub fn read_import_headers(
    request: &ReadImportHeadersRequest,
) -> Result<ImportHeaders, ImportError> {
    let parsed = parse_csv(&request.csv_text)?;

    Ok(ImportHeaders {
        import_type: request.import_type,
        file_name: request.file_name.clone(),
        delimiter: delimiter_label(parsed.delimiter),
        headers: parsed.headers,
        total_rows: parsed.rows.len(),
    })
}

pub fn validate_import(
    db: &Db,
    request: &ValidateImportRequest,
) -> Result<ImportValidationResult, ImportError> {
    let parsed = parse_csv(&request.csv_text)?;
    let lookup = HeaderLookup::new(&parsed.headers);
    let connection = db.open()?;
    let rows = match request.import_type {
        ImportType::Products => {
            validate_product_rows(&connection, &parsed, &lookup, &request.mapping)?
        }
        ImportType::Categories => {
            validate_category_rows(&connection, &parsed, &lookup, &request.mapping)?
        }
        ImportType::InitialStock => {
            validate_initial_stock_rows(&connection, &parsed, &lookup, &request.mapping)?
        }
    };

    Ok(validation_result(
        request.import_type,
        &request.file_name,
        parsed.rows.len(),
        rows,
    ))
}

pub fn commit_import(
    db: &Db,
    request: &CommitImportRequest,
) -> Result<ImportJobSummary, ImportError> {
    let validation = validate_import(
        db,
        &ValidateImportRequest {
            import_type: request.import_type,
            file_name: request.file_name.clone(),
            csv_text: request.csv_text.clone(),
            mapping: request.mapping.clone(),
        },
    )?;

    if validation.error_count > 0 {
        return Err(ImportError::with_details(
            "import_row_errors",
            "Import ima greske. Ispravite oznacene redove pre upisa.",
            serde_json::to_value(&validation)?,
        ));
    }

    let parsed = parse_csv(&request.csv_text)?;
    let lookup = HeaderLookup::new(&parsed.headers);
    let mut connection = db.open()?;
    let tx = connection.transaction()?;
    let now = now_utc_string()?;
    let mapping_json = serde_json::to_string(&request.mapping)?;

    tx.execute(
        "INSERT INTO import_jobs (
            import_type,
            file_name,
            column_mapping_json,
            status,
            total_rows,
            error_rows,
            created_at,
            completed_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
        params![
            request.import_type.as_db_str(),
            request.file_name,
            mapping_json,
            ImportJobStatus::Completed.as_db_str(),
            parsed.rows.len() as i64,
            now
        ],
    )?;
    let job_id = tx.last_insert_rowid();

    match request.import_type {
        ImportType::Products => commit_product_rows(
            &tx,
            job_id,
            &parsed,
            &lookup,
            &request.mapping,
            &validation.rows,
        )?,
        ImportType::Categories => commit_category_rows(
            &tx,
            job_id,
            &parsed,
            &lookup,
            &request.mapping,
            &validation.rows,
        )?,
        ImportType::InitialStock => commit_initial_stock_rows(
            &tx,
            job_id,
            &parsed,
            &lookup,
            &request.mapping,
            &validation.rows,
        )?,
    }

    tx.commit()?;

    Ok(ImportJobSummary {
        id: job_id,
        import_type: request.import_type,
        file_name: request.file_name.clone(),
        status: ImportJobStatus::Completed,
        total_rows: parsed.rows.len(),
        error_rows: 0,
        created_at: now.clone(),
        completed_at: Some(now),
    })
}

pub fn list_import_jobs(db: &Db) -> Result<Vec<ImportJobSummary>, ImportError> {
    let connection = db.open()?;
    let mut statement = connection.prepare(
        "SELECT id, import_type, file_name, status, total_rows, error_rows, created_at, completed_at
         FROM import_jobs
         ORDER BY id DESC",
    )?;
    let rows = statement.query_map([], import_job_summary_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_import_job(db: &Db, id: i64) -> Result<ImportJobDetail, ImportError> {
    let connection = db.open()?;
    let summary = connection
        .query_row(
            "SELECT id, import_type, file_name, status, total_rows, error_rows, created_at, completed_at
             FROM import_jobs
             WHERE id = ?1",
            params![id],
            import_job_summary_from_row,
        )
        .optional()?
        .ok_or_else(|| ImportError::new("not_found", "Import posao nije pronadjen."))?;

    let mut statement = connection.prepare(
        "SELECT row_number, raw_json, status, message
         FROM import_job_rows
         WHERE import_job_id = ?1
         ORDER BY row_number ASC",
    )?;
    let rows = statement
        .query_map(params![id], |row| {
            let raw_json: String = row.get(1)?;
            let values = serde_json::from_str(&raw_json).unwrap_or_default();
            Ok(ImportJobRowDetail {
                row_number: row.get::<_, i64>(0)? as usize,
                values,
                status: parse_row_status(row.get::<_, String>(2)?.as_str()),
                message: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ImportJobDetail {
        id: summary.id,
        import_type: summary.import_type,
        file_name: summary.file_name,
        status: summary.status,
        total_rows: summary.total_rows,
        error_rows: summary.error_rows,
        created_at: summary.created_at,
        completed_at: summary.completed_at,
        rows,
    })
}

fn import_job_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportJobSummary> {
    Ok(ImportJobSummary {
        id: row.get(0)?,
        import_type: parse_import_type(row.get::<_, String>(1)?.as_str()),
        file_name: row.get(2)?,
        status: parse_job_status(row.get::<_, String>(3)?.as_str()),
        total_rows: row.get::<_, i64>(4)? as usize,
        error_rows: row.get::<_, i64>(5)? as usize,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
    })
}

fn validate_product_rows(
    connection: &Connection,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
) -> Result<Vec<ImportRowResult>, ImportError> {
    if missing(mapping, "name")
        || missing(mapping, "sale_price")
        || missing(mapping, "vat_rate")
        || (missing(mapping, "sku") && missing(mapping, "barcode"))
    {
        return Ok(vec![mapping_error(
            "Mapirajte prodajnu cenu, PDV stopu i sifru ili barcode.",
        )]);
    }

    let mut rows = Vec::with_capacity(parsed.rows.len());
    let mut seen_skus = HashSet::new();
    let mut seen_barcodes = HashSet::new();

    for (index, row) in parsed.rows.iter().enumerate() {
        let row_number = index + 2;
        let values = row_values(&parsed.headers, row);
        let name = required_value(lookup, row, mapping, "name");
        let sku = optional_value(lookup, row, mapping, "sku");
        let barcode = optional_value(lookup, row, mapping, "barcode");

        let mut status = ImportRowStatus::Valid;
        let mut action = ImportRowAction::Create;
        let mut message = String::new();

        if name.is_empty() {
            status = ImportRowStatus::Error;
            message = "Naziv je obavezan.".to_string();
        } else if sku.is_empty() && barcode.is_empty() {
            status = ImportRowStatus::Error;
            message = "Unesite sifru ili barcode.".to_string();
        } else if !sku.is_empty() && !seen_skus.insert(normalize_key(&sku)) {
            status = ImportRowStatus::Error;
            message = "Duplikat sifre u fajlu.".to_string();
        } else if !barcode.is_empty() && !seen_barcodes.insert(normalize_key(&barcode)) {
            status = ImportRowStatus::Error;
            message = "Duplikat barcode-a u fajlu.".to_string();
        } else if parse_money_minor(&required_value(lookup, row, mapping, "sale_price")).is_err() {
            status = ImportRowStatus::Error;
            message = "Cena nije ispravna.".to_string();
        } else if parse_optional_money(lookup, row, mapping, "purchase_price").is_err() {
            status = ImportRowStatus::Error;
            message = "Nabavna cena nije ispravna.".to_string();
        } else if parse_optional_quantity(lookup, row, mapping, "minimum_stock", true).is_err() {
            status = ImportRowStatus::Error;
            message = "Minimalna zaliha nije ispravna.".to_string();
        } else if parse_optional_quantity(lookup, row, mapping, "initial_stock", true).is_err() {
            status = ImportRowStatus::Error;
            message = "Pocetna zaliha nije ispravna.".to_string();
        } else if find_tax_rate_id(
            connection,
            &required_value(lookup, row, mapping, "vat_rate"),
        )?
        .is_none()
        {
            status = ImportRowStatus::Error;
            message = "PDV stopa nije pronadjena.".to_string();
        } else if let Some(existing_id) = find_existing_product(connection, &sku, &barcode)? {
            status = ImportRowStatus::Warning;
            action = ImportRowAction::Update;
            message = format!("Postojeci artikal #{existing_id} bice azuriran.");
        } else if product_name_exists(connection, &name)? {
            status = ImportRowStatus::Warning;
            message = "Naziv vec postoji; proverite da nije duplikat.".to_string();
        }

        rows.push(ImportRowResult {
            row_number,
            status,
            action,
            message,
            values,
        });
    }

    Ok(rows)
}

fn validate_category_rows(
    connection: &Connection,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
) -> Result<Vec<ImportRowResult>, ImportError> {
    if missing(mapping, "name") {
        return Ok(vec![mapping_error("Mapirajte naziv kategorije.")]);
    }

    let mut seen_names = HashSet::new();
    let mut rows = Vec::with_capacity(parsed.rows.len());

    for (index, row) in parsed.rows.iter().enumerate() {
        let name = required_value(lookup, row, mapping, "name");
        let mut status = ImportRowStatus::Valid;
        let mut action = ImportRowAction::Create;
        let mut message = String::new();

        if name.is_empty() {
            status = ImportRowStatus::Error;
            message = "Naziv kategorije je obavezan.".to_string();
        } else if !seen_names.insert(normalize_key(&name)) {
            status = ImportRowStatus::Error;
            message = "Duplikat kategorije u fajlu.".to_string();
        } else if category_id_by_name(connection, &name)?.is_some() {
            status = ImportRowStatus::Warning;
            action = ImportRowAction::Update;
            message = "Postojeca kategorija bice azurirana.".to_string();
        }

        rows.push(ImportRowResult {
            row_number: index + 2,
            status,
            action,
            message,
            values: row_values(&parsed.headers, row),
        });
    }

    Ok(rows)
}

fn validate_initial_stock_rows(
    connection: &Connection,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
) -> Result<Vec<ImportRowResult>, ImportError> {
    if missing(mapping, "quantity") || (missing(mapping, "sku") && missing(mapping, "barcode")) {
        return Ok(vec![mapping_error(
            "Mapirajte kolicinu i sifru ili barcode artikla.",
        )]);
    }

    let mut rows = Vec::with_capacity(parsed.rows.len());

    for (index, row) in parsed.rows.iter().enumerate() {
        let sku = optional_value(lookup, row, mapping, "sku");
        let barcode = optional_value(lookup, row, mapping, "barcode");
        let quantity = required_value(lookup, row, mapping, "quantity");
        let mut status = ImportRowStatus::Valid;
        let action = ImportRowAction::Update;
        let mut message = String::new();

        if sku.is_empty() && barcode.is_empty() {
            status = ImportRowStatus::Error;
            message = "Unesite sifru ili barcode.".to_string();
        } else if parse_quantity_milli(&quantity, false).is_err() {
            status = ImportRowStatus::Error;
            message = "Kolicina nije ispravna.".to_string();
        } else if find_existing_product(connection, &sku, &barcode)?.is_none() {
            status = ImportRowStatus::Error;
            message = "Artikal nije pronadjen.".to_string();
        }

        rows.push(ImportRowResult {
            row_number: index + 2,
            status,
            action,
            message,
            values: row_values(&parsed.headers, row),
        });
    }

    Ok(rows)
}

fn commit_product_rows(
    tx: &rusqlite::Transaction<'_>,
    job_id: i64,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
    validated_rows: &[ImportRowResult],
) -> Result<(), ImportError> {
    for (index, row) in parsed.rows.iter().enumerate() {
        let sku_input = optional_value(lookup, row, mapping, "sku");
        let barcode_input = optional_value(lookup, row, mapping, "barcode");
        let sku = if sku_input.is_empty() {
            barcode_input.clone()
        } else {
            sku_input.clone()
        };
        let barcode = empty_to_none(barcode_input);
        let name = required_value(lookup, row, mapping, "name");
        let category_id = match optional_value(lookup, row, mapping, "category") {
            category if category.is_empty() => None,
            category => Some(find_or_create_category(tx, &category)?),
        };
        let sale_price = parse_money_minor(&required_value(lookup, row, mapping, "sale_price"))?;
        let purchase_price =
            parse_optional_money(lookup, row, mapping, "purchase_price")?.unwrap_or(0);
        let tax_rate_id = find_tax_rate_id(tx, &required_value(lookup, row, mapping, "vat_rate"))?
            .ok_or_else(|| ImportError::new("validation_error", "PDV stopa nije pronadjena."))?;
        let minimum_stock =
            parse_optional_quantity(lookup, row, mapping, "minimum_stock", true)?.unwrap_or(0);
        let unit = optional_value(lookup, row, mapping, "unit_of_measure");
        let unit = if unit.is_empty() {
            "kom".to_string()
        } else {
            unit
        };
        let now = now_utc_string()?;
        let product_id = if let Some(product_id) =
            find_existing_product(tx, &sku_input, barcode.as_deref().unwrap_or(""))?
        {
            tx.execute(
                "UPDATE products
                 SET name = ?1,
                     sku = ?2,
                     barcode = ?3,
                     category_id = ?4,
                     unit_of_measure = ?5,
                     sale_price_minor = ?6,
                     purchase_price_minor = ?7,
                     tax_rate_id = ?8,
                     minimum_stock_milli = ?9,
                     updated_at = ?10
                 WHERE id = ?11",
                params![
                    name,
                    sku,
                    barcode,
                    category_id,
                    unit,
                    sale_price,
                    purchase_price,
                    tax_rate_id,
                    minimum_stock,
                    now,
                    product_id
                ],
            )?;
            product_id
        } else {
            tx.execute(
                "INSERT INTO products (
                    name,
                    sku,
                    barcode,
                    category_id,
                    unit_of_measure,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    created_at,
                    updated_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    name,
                    sku,
                    barcode,
                    category_id,
                    unit,
                    sale_price,
                    purchase_price,
                    tax_rate_id,
                    minimum_stock,
                    now
                ],
            )?;
            tx.last_insert_rowid()
        };

        if let Some(initial_stock) =
            parse_optional_quantity(lookup, row, mapping, "initial_stock", true)?
        {
            if initial_stock > 0 {
                apply_stock_delta(tx, product_id, initial_stock, "Pocetno stanje iz importa")?;
            }
        }

        insert_job_row(tx, job_id, &parsed.headers, row, &validated_rows[index])?;
    }

    Ok(())
}

fn commit_category_rows(
    tx: &rusqlite::Transaction<'_>,
    job_id: i64,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
    validated_rows: &[ImportRowResult],
) -> Result<(), ImportError> {
    for (index, row) in parsed.rows.iter().enumerate() {
        let name = required_value(lookup, row, mapping, "name");
        let active = parse_boolish(&optional_value(lookup, row, mapping, "active")).unwrap_or(true);
        let now = now_utc_string()?;

        if let Some(category_id) = category_id_by_name(tx, &name)? {
            tx.execute(
                "UPDATE categories SET name = ?1, active = ?2, updated_at = ?3 WHERE id = ?4",
                params![name, bool_to_i64(active), now, category_id],
            )?;
        } else {
            tx.execute(
                "INSERT INTO categories (name, active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![name, bool_to_i64(active), now],
            )?;
        }

        insert_job_row(tx, job_id, &parsed.headers, row, &validated_rows[index])?;
    }

    Ok(())
}

fn commit_initial_stock_rows(
    tx: &rusqlite::Transaction<'_>,
    job_id: i64,
    parsed: &ParsedCsv,
    lookup: &HeaderLookup,
    mapping: &HashMap<String, String>,
    validated_rows: &[ImportRowResult],
) -> Result<(), ImportError> {
    for (index, row) in parsed.rows.iter().enumerate() {
        let sku = optional_value(lookup, row, mapping, "sku");
        let barcode = optional_value(lookup, row, mapping, "barcode");
        let quantity =
            parse_quantity_milli(&required_value(lookup, row, mapping, "quantity"), false)?;
        let product_id = find_existing_product(tx, &sku, &barcode)
            .map_err(ImportError::from)?
            .ok_or_else(|| ImportError::new("validation_error", "Artikal nije pronadjen."))?;
        apply_stock_delta(tx, product_id, quantity, "Pocetno stanje iz importa")?;
        insert_job_row(tx, job_id, &parsed.headers, row, &validated_rows[index])?;
    }

    Ok(())
}

fn apply_stock_delta(
    connection: &Connection,
    product_id: i64,
    quantity_milli: i64,
    reason: &str,
) -> Result<(), ImportError> {
    let now = now_utc_string()?;
    connection.execute(
        "INSERT INTO inventory_movements (
            product_id,
            movement_type,
            quantity_milli,
            reason,
            reference_type,
            created_at
         )
         VALUES (?1, 'receive', ?2, ?3, 'import', ?4)",
        params![product_id, quantity_milli, reason, now],
    )?;
    connection.execute(
        "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(product_id) DO UPDATE
         SET quantity_milli = quantity_milli + excluded.quantity_milli,
             updated_at = excluded.updated_at",
        params![product_id, quantity_milli, now],
    )?;
    Ok(())
}

fn insert_job_row(
    connection: &Connection,
    job_id: i64,
    headers: &[String],
    row: &[String],
    validation: &ImportRowResult,
) -> Result<(), ImportError> {
    let raw_json = serde_json::to_string(&row_values(headers, row))?;
    let status = if validation.status == ImportRowStatus::Error {
        ImportRowStatus::Skipped
    } else {
        ImportRowStatus::Imported
    };
    let message = if validation.message.is_empty() {
        None
    } else {
        Some(validation.message.as_str())
    };

    connection.execute(
        "INSERT INTO import_job_rows (import_job_id, row_number, raw_json, status, message)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            job_id,
            validation.row_number as i64,
            raw_json,
            status.as_db_str(),
            message
        ],
    )?;
    Ok(())
}

fn validation_result(
    import_type: ImportType,
    file_name: &str,
    total_rows: usize,
    rows: Vec<ImportRowResult>,
) -> ImportValidationResult {
    let mut summary = ImportSummary {
        create: 0,
        update: 0,
        skip: 0,
    };
    let mut valid_count = 0;
    let mut warning_count = 0;
    let mut error_count = 0;

    for row in &rows {
        match row.status {
            ImportRowStatus::Valid => valid_count += 1,
            ImportRowStatus::Warning => warning_count += 1,
            ImportRowStatus::Error => error_count += 1,
            ImportRowStatus::Imported | ImportRowStatus::Skipped => {}
        }

        if row.status == ImportRowStatus::Error {
            summary.skip += 1;
        } else {
            match row.action {
                ImportRowAction::Create => summary.create += 1,
                ImportRowAction::Update => summary.update += 1,
                ImportRowAction::Skip => summary.skip += 1,
            }
        }
    }

    ImportValidationResult {
        import_type,
        file_name: file_name.to_string(),
        total_rows,
        valid_count,
        warning_count,
        error_count,
        summary,
        rows,
    }
}

fn parse_csv(text: &str) -> Result<ParsedCsv, ImportError> {
    let trimmed = text.trim_start_matches('\u{feff}');
    let first_line = trimmed
        .lines()
        .next()
        .ok_or_else(|| ImportError::new("validation_error", "CSV fajl je prazan."))?;
    let delimiter = detect_delimiter(first_line);
    let records = parse_csv_records(trimmed, delimiter)?;
    let mut records = records
        .into_iter()
        .filter(|record| record.iter().any(|value| !value.trim().is_empty()));
    let headers = records
        .next()
        .ok_or_else(|| ImportError::new("validation_error", "CSV nema zaglavlje."))?
        .into_iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();

    if headers.is_empty() || headers.iter().all(String::is_empty) {
        return Err(ImportError::new("validation_error", "CSV nema zaglavlje."));
    }

    let rows = records
        .map(|record| {
            record
                .into_iter()
                .map(|value| value.trim().to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(ParsedCsv {
        delimiter,
        headers,
        rows,
    })
}

fn parse_csv_records(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, ImportError> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                field.push('"');
                let _ = chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if ch == delimiter && !in_quotes {
            record.push(field);
            field = String::new();
        } else if (ch == '\n' || ch == '\r') && !in_quotes {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                let _ = chars.next();
            }
            record.push(field);
            field = String::new();
            records.push(record);
            record = Vec::new();
        } else {
            field.push(ch);
        }
    }

    if in_quotes {
        return Err(ImportError::new(
            "validation_error",
            "CSV navodnici nisu zatvoreni.",
        ));
    }

    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }

    Ok(records)
}

fn detect_delimiter(line: &str) -> char {
    [';', ',', '\t']
        .into_iter()
        .max_by_key(|candidate| line.matches(*candidate).count())
        .unwrap_or(';')
}

fn delimiter_label(delimiter: char) -> String {
    if delimiter == '\t' {
        "\\t".to_string()
    } else {
        delimiter.to_string()
    }
}

fn missing(mapping: &HashMap<String, String>, field: &str) -> bool {
    mapping
        .get(field)
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
}

fn mapping_error(message: &str) -> ImportRowResult {
    ImportRowResult {
        row_number: 0,
        status: ImportRowStatus::Error,
        action: ImportRowAction::Skip,
        message: message.to_string(),
        values: HashMap::new(),
    }
}

fn required_value(
    lookup: &HeaderLookup,
    row: &[String],
    mapping: &HashMap<String, String>,
    field: &str,
) -> String {
    optional_value(lookup, row, mapping, field)
}

fn optional_value(
    lookup: &HeaderLookup,
    row: &[String],
    mapping: &HashMap<String, String>,
    field: &str,
) -> String {
    lookup
        .value(row, mapping, field)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn row_values(headers: &[String], row: &[String]) -> HashMap<String, String> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            (
                header.clone(),
                row.get(index)
                    .cloned()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            )
        })
        .collect()
}

fn parse_money_minor(input: &str) -> Result<i64, ImportError> {
    let compact = input.trim().replace(' ', "");

    if compact.is_empty() {
        return Err(ImportError::new("validation_error", "Iznos nije ispravan."));
    }

    let normalized = normalize_decimal(&compact)?;
    let value = decimal_to_scaled_int(&normalized, 2)?;

    if value < 0 {
        return Err(ImportError::new("validation_error", "Iznos nije ispravan."));
    }

    Ok(value)
}

fn parse_optional_money(
    lookup: &HeaderLookup,
    row: &[String],
    mapping: &HashMap<String, String>,
    field: &str,
) -> Result<Option<i64>, ImportError> {
    let value = optional_value(lookup, row, mapping, field);

    if value.is_empty() {
        Ok(None)
    } else {
        parse_money_minor(&value).map(Some)
    }
}

fn parse_quantity_milli(input: &str, allow_zero: bool) -> Result<i64, ImportError> {
    let compact = input.trim().replace(' ', "");

    if compact.is_empty() {
        return Err(ImportError::new(
            "validation_error",
            "Kolicina nije ispravna.",
        ));
    }

    let normalized = normalize_decimal(&compact)?;
    let quantity = decimal_to_scaled_int(&normalized, 3)?;

    if quantity < 0 || (!allow_zero && quantity == 0) {
        return Err(ImportError::new(
            "validation_error",
            "Kolicina nije ispravna.",
        ));
    }

    Ok(quantity)
}

fn parse_optional_quantity(
    lookup: &HeaderLookup,
    row: &[String],
    mapping: &HashMap<String, String>,
    field: &str,
    allow_zero: bool,
) -> Result<Option<i64>, ImportError> {
    let value = optional_value(lookup, row, mapping, field);

    if value.is_empty() {
        Ok(None)
    } else {
        parse_quantity_milli(&value, allow_zero).map(Some)
    }
}

fn normalize_decimal(input: &str) -> Result<String, ImportError> {
    let without_percent = input.trim_end_matches('%');

    if without_percent.contains(',') {
        Ok(without_percent.replace('.', "").replace(',', "."))
    } else if is_grouped_integer(without_percent) {
        Ok(without_percent.replace('.', ""))
    } else {
        Ok(without_percent.to_string())
    }
}

fn decimal_to_scaled_int(input: &str, scale: usize) -> Result<i64, ImportError> {
    let negative = input.starts_with('-');
    let unsigned = input.trim_start_matches('-');
    let parts = unsigned.split('.').collect::<Vec<_>>();

    if parts.len() > 2
        || parts[0].is_empty()
        || !parts[0].chars().all(|ch| ch.is_ascii_digit())
        || parts
            .get(1)
            .map(|part| part.len() > scale || !part.chars().all(|ch| ch.is_ascii_digit()))
            .unwrap_or(false)
    {
        return Err(ImportError::new("validation_error", "Broj nije ispravan."));
    }

    let whole = parts[0]
        .parse::<i64>()
        .map_err(|_| ImportError::new("validation_error", "Broj nije ispravan."))?;
    let fraction = parts.get(1).copied().unwrap_or("");
    let fraction_value = format!("{fraction:0<scale$}")
        .parse::<i64>()
        .map_err(|_| ImportError::new("validation_error", "Broj nije ispravan."))?;
    let multiplier = 10_i64.pow(scale as u32);
    let value = whole
        .checked_mul(multiplier)
        .and_then(|scaled| scaled.checked_add(fraction_value))
        .ok_or_else(|| ImportError::new("validation_error", "Broj nije ispravan."))?;

    Ok(if negative { -value } else { value })
}

fn is_grouped_integer(input: &str) -> bool {
    let groups = input.split('.').collect::<Vec<_>>();
    groups.len() > 1
        && !groups[0].is_empty()
        && groups[0].len() <= 3
        && groups[0].chars().all(|ch| ch.is_ascii_digit())
        && groups[1..]
            .iter()
            .all(|group| group.len() == 3 && group.chars().all(|ch| ch.is_ascii_digit()))
}

fn find_tax_rate_id(connection: &Connection, input: &str) -> Result<Option<i64>, ImportError> {
    let normalized = normalize_key(input);
    let parsed_basis_points = parse_vat_basis_points(input).ok();
    let mut statement = connection.prepare(
        "SELECT id, name, rate_basis_points
         FROM tax_rates
         WHERE active = 1",
    )?;
    let rates = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    for rate in rates {
        let (id, name, basis_points) = rate?;
        if parsed_basis_points == Some(basis_points) || normalize_key(&name) == normalized {
            return Ok(Some(id));
        }
    }

    Ok(None)
}

fn parse_vat_basis_points(input: &str) -> Result<i64, ImportError> {
    let normalized = normalize_decimal(input)?;
    decimal_to_scaled_int(&normalized, 2)
}

fn find_existing_product(
    connection: &Connection,
    sku: &str,
    barcode: &str,
) -> Result<Option<i64>, rusqlite::Error> {
    if !barcode.trim().is_empty() {
        if let Some(id) = connection
            .query_row(
                "SELECT id FROM products WHERE barcode = ?1",
                params![barcode.trim()],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(Some(id));
        }
    }

    if !sku.trim().is_empty() {
        return connection
            .query_row(
                "SELECT id FROM products WHERE sku = ?1",
                params![sku.trim()],
                |row| row.get(0),
            )
            .optional();
    }

    Ok(None)
}

fn product_name_exists(connection: &Connection, name: &str) -> Result<bool, ImportError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM products WHERE lower(name) = lower(?1)",
        params![name.trim()],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

fn category_id_by_name(connection: &Connection, name: &str) -> Result<Option<i64>, ImportError> {
    connection
        .query_row(
            "SELECT id FROM categories WHERE lower(name) = lower(?1)",
            params![name.trim()],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn find_or_create_category(connection: &Connection, name: &str) -> Result<i64, ImportError> {
    if let Some(id) = category_id_by_name(connection, name)? {
        return Ok(id);
    }

    let now = now_utc_string()?;
    connection.execute(
        "INSERT INTO categories (name, active, created_at, updated_at)
         VALUES (?1, 1, ?2, ?2)",
        params![name.trim(), now],
    )?;
    Ok(connection.last_insert_rowid())
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn parse_boolish(value: &str) -> Option<bool> {
    match normalize_key(value).as_str() {
        "" => None,
        "1" | "true" | "da" | "yes" | "active" | "aktivan" => Some(true),
        "0" | "false" | "ne" | "no" | "inactive" | "neaktivan" => Some(false),
        _ => None,
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn now_utc_string() -> Result<String, ImportError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(Into::into)
}

fn parse_import_type(value: &str) -> ImportType {
    match value {
        "categories" => ImportType::Categories,
        "initial_stock" => ImportType::InitialStock,
        _ => ImportType::Products,
    }
}

fn parse_job_status(value: &str) -> ImportJobStatus {
    match value {
        "draft" => ImportJobStatus::Draft,
        "validated" => ImportJobStatus::Validated,
        "failed" => ImportJobStatus::Failed,
        _ => ImportJobStatus::Completed,
    }
}

fn parse_row_status(value: &str) -> ImportRowStatus {
    match value {
        "valid" => ImportRowStatus::Valid,
        "warning" => ImportRowStatus::Warning,
        "error" => ImportRowStatus::Error,
        "skipped" => ImportRowStatus::Skipped,
        _ => ImportRowStatus::Imported,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rusqlite::params;

    use super::*;
    use crate::db::{test_database_path, Db};

    fn mapping(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(field, header)| ((*field).to_string(), (*header).to_string()))
            .collect()
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

    fn seed_tax_rate(db: &Db, name: &str, basis_points: i64) {
        let connection = db.open().expect("database should open");
        connection
            .execute(
                "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES (?1, ?2, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![name, basis_points],
            )
            .expect("tax rate should insert");
    }

    fn seed_product(db: &Db, sku: &str, barcode: &str) -> i64 {
        seed_tax_rate(db, "PDV 20", 2000);
        let connection = db.open().expect("database should open");
        let tax_rate_id: i64 = connection
            .query_row(
                "SELECT id FROM tax_rates WHERE rate_basis_points = 2000",
                [],
                |row| row.get(0),
            )
            .expect("tax rate id should query");

        connection
            .execute(
                "INSERT INTO products (
                    name,
                    sku,
                    barcode,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    created_at,
                    updated_at
                 )
                 VALUES ('Postojeci artikal', ?1, ?2, 10000, 7000, ?3, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![sku, barcode, tax_rate_id],
            )
            .expect("product should insert");

        connection.last_insert_rowid()
    }

    #[test]
    fn read_headers_detects_semicolon_csv_and_counts_data_rows() {
        let request = ReadImportHeadersRequest {
            import_type: ImportType::Products,
            file_name: "artikli.csv".to_string(),
            csv_text: "Naziv;Cena;PDV\nHleb;120,00;20\nMleko;150,50;20\n".to_string(),
        };

        let headers = read_import_headers(&request).expect("headers should parse");

        assert_eq!(
            headers,
            ImportHeaders {
                import_type: ImportType::Products,
                file_name: "artikli.csv".to_string(),
                delimiter: ";".to_string(),
                headers: vec!["Naziv".to_string(), "Cena".to_string(), "PDV".to_string()],
                total_rows: 2,
            }
        );
    }

    #[test]
    fn validate_products_requires_sale_price_vat_and_sku_or_barcode_mapping() {
        with_test_database(
            "validate_products_requires_sale_price_vat_and_sku_or_barcode_mapping",
            |db| {
                let request = ValidateImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text: "Naziv;Cena;PDV\nHleb;120,00;20\n".to_string(),
                    mapping: mapping(&[("name", "Naziv")]),
                };

                let result = validate_import(db, &request).expect("validation should run");

                assert_eq!(result.error_count, 1);
                assert_eq!(
                    result.rows[0].message,
                    "Mapirajte prodajnu cenu, PDV stopu i sifru ili barcode."
                );
            },
        );
    }

    #[test]
    fn validate_products_reports_invalid_money_with_row_number() {
        with_test_database(
            "validate_products_reports_invalid_money_with_row_number",
            |db| {
                seed_tax_rate(db, "PDV 20", 2000);
                let request = ValidateImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text: "Naziv;Cena;PDV;Sifra\nHleb;nije-cena;20;SKU-1\n".to_string(),
                    mapping: mapping(&[
                        ("name", "Naziv"),
                        ("sale_price", "Cena"),
                        ("vat_rate", "PDV"),
                        ("sku", "Sifra"),
                    ]),
                };

                let result = validate_import(db, &request).expect("validation should run");

                assert_eq!(result.rows[0].row_number, 2);
                assert_eq!(result.rows[0].status, ImportRowStatus::Error);
                assert_eq!(result.rows[0].message, "Cena nije ispravna.");
            },
        );
    }

    #[test]
    fn validate_products_warns_when_existing_barcode_will_be_updated() {
        with_test_database(
            "validate_products_warns_when_existing_barcode_will_be_updated",
            |db| {
                seed_product(db, "SKU-OLD", "8600001");
                let request = ValidateImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text: "Naziv;Cena;PDV;Barcode\nNovi naziv;130,00;20;8600001\n".to_string(),
                    mapping: mapping(&[
                        ("name", "Naziv"),
                        ("sale_price", "Cena"),
                        ("vat_rate", "PDV"),
                        ("barcode", "Barcode"),
                    ]),
                };

                let result = validate_import(db, &request).expect("validation should run");

                assert_eq!(result.warning_count, 1);
                assert_eq!(result.rows[0].status, ImportRowStatus::Warning);
                assert_eq!(result.rows[0].action, ImportRowAction::Update);
            },
        );
    }

    #[test]
    fn commit_products_writes_product_stock_job_and_history_transactionally() {
        with_test_database(
            "commit_products_writes_product_stock_job_and_history_transactionally",
            |db| {
                seed_tax_rate(db, "PDV 20", 2000);
                let request = CommitImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text:
                        "Naziv;Cena;PDV;Sifra;Barcode;Zaliha\nHleb;120,00;20;SKU-1;8600001;3,5\n"
                            .to_string(),
                    mapping: mapping(&[
                        ("name", "Naziv"),
                        ("sale_price", "Cena"),
                        ("vat_rate", "PDV"),
                        ("sku", "Sifra"),
                        ("barcode", "Barcode"),
                        ("initial_stock", "Zaliha"),
                    ]),
                };

                let result = commit_import(db, &request).expect("commit should succeed");

                let connection = db.open().expect("database should open");
                let product_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM products WHERE sku = 'SKU-1'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("product count should query");
                let balance: i64 = connection
                    .query_row(
                        "SELECT quantity_milli FROM inventory_balances
                         JOIN products ON products.id = inventory_balances.product_id
                         WHERE products.sku = 'SKU-1'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("balance should query");

                assert_eq!(result.status, ImportJobStatus::Completed);
                assert_eq!(product_count, 1);
                assert_eq!(balance, 3500);
                assert_eq!(
                    list_import_jobs(db).expect("history should load")[0].id,
                    result.id
                );
            },
        );
    }

    #[test]
    fn commit_rejects_invalid_rows_without_partial_product_writes() {
        with_test_database(
            "commit_rejects_invalid_rows_without_partial_product_writes",
            |db| {
                seed_tax_rate(db, "PDV 20", 2000);
                let request = CommitImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text:
                        "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\nMleko;neispravno;20;SKU-2\n"
                            .to_string(),
                    mapping: mapping(&[
                        ("name", "Naziv"),
                        ("sale_price", "Cena"),
                        ("vat_rate", "PDV"),
                        ("sku", "Sifra"),
                    ]),
                };

                let error = commit_import(db, &request).expect_err("commit should fail");
                let connection = db.open().expect("database should open");
                let product_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))
                    .expect("product count should query");

                assert_eq!(error.code(), "import_row_errors");
                assert_eq!(product_count, 0);
            },
        );
    }

    #[test]
    fn commit_initial_stock_writes_receive_movement_for_existing_sku() {
        with_test_database(
            "commit_initial_stock_writes_receive_movement_for_existing_sku",
            |db| {
                let product_id = seed_product(db, "SKU-STOCK", "8600002");
                let request = CommitImportRequest {
                    import_type: ImportType::InitialStock,
                    file_name: "stanje.csv".to_string(),
                    csv_text: "Sifra;Kolicina\nSKU-STOCK;2\n".to_string(),
                    mapping: mapping(&[("sku", "Sifra"), ("quantity", "Kolicina")]),
                };

                commit_import(db, &request).expect("initial stock should commit");

                let connection = db.open().expect("database should open");
                let movement_quantity: i64 = connection
                    .query_row(
                        "SELECT quantity_milli FROM inventory_movements
                         WHERE product_id = ?1 AND movement_type = 'receive'",
                        params![product_id],
                        |row| row.get(0),
                    )
                    .expect("movement should query");

                assert_eq!(movement_quantity, 2000);
            },
        );
    }

    #[test]
    fn validate_products_flags_unknown_vat_rate_as_error() {
        with_test_database("validate_products_flags_unknown_vat_rate_as_error", |db| {
            // No tax rate seeded, so the 20% column cannot resolve to a tax_rates row.
            let request = ValidateImportRequest {
                import_type: ImportType::Products,
                file_name: "artikli.csv".to_string(),
                csv_text: "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\n".to_string(),
                mapping: mapping(&[
                    ("name", "Naziv"),
                    ("sale_price", "Cena"),
                    ("vat_rate", "PDV"),
                    ("sku", "Sifra"),
                ]),
            };

            let result = validate_import(db, &request).expect("validation should run");

            assert_eq!(result.error_count, 1);
            assert_eq!(result.rows[0].row_number, 2);
            assert_eq!(result.rows[0].status, ImportRowStatus::Error);
            assert_eq!(result.rows[0].message, "PDV stopa nije pronadjena.");
        });
    }

    #[test]
    fn validate_products_accepts_known_vat_rate() {
        with_test_database("validate_products_accepts_known_vat_rate", |db| {
            seed_tax_rate(db, "PDV 20", 2000);
            let request = ValidateImportRequest {
                import_type: ImportType::Products,
                file_name: "artikli.csv".to_string(),
                csv_text: "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\n".to_string(),
                mapping: mapping(&[
                    ("name", "Naziv"),
                    ("sale_price", "Cena"),
                    ("vat_rate", "PDV"),
                    ("sku", "Sifra"),
                ]),
            };

            let result = validate_import(db, &request).expect("validation should run");

            assert_eq!(result.error_count, 0);
            assert_eq!(result.rows[0].status, ImportRowStatus::Valid);
            assert_eq!(result.rows[0].action, ImportRowAction::Create);
        });
    }

    #[test]
    fn validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping() {
        with_test_database(
            "validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping",
            |db| {
                let request = ValidateImportRequest {
                    import_type: ImportType::InitialStock,
                    file_name: "stanje.csv".to_string(),
                    csv_text: "Sifra;Kolicina\nSKU-1;2\n".to_string(),
                    mapping: mapping(&[("sku", "Sifra")]),
                };

                let result = validate_import(db, &request).expect("validation should run");

                assert_eq!(result.error_count, 1);
                assert_eq!(
                    result.rows[0].message,
                    "Mapirajte kolicinu i sifru ili barcode artikla."
                );
            },
        );
    }
}
