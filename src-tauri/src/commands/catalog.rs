use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::db::Db;
use crate::state::AppState;

const DEFAULT_PRODUCT_LIST_LIMIT: i64 = 500;
const DEFAULT_PRODUCT_SEARCH_LIMIT: i64 = 20;
const MAX_PRODUCT_LIST_LIMIT: i64 = 500;
const OPEN_FOOD_FACTS_USER_AGENT: &str = "VantumPOS/0.1 (office@actaer.com)";
const OPEN_FOOD_FACTS_TIMEOUT_SECONDS: u64 = 8;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductListQuery {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub tax_rate_id: Option<i64>,
    pub active: Option<bool>,
    pub low_stock: Option<bool>,
    pub missing_barcode: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSearchQuery {
    pub search: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProductRequest {
    pub name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub category_id: Option<i64>,
    pub unit_of_measure: String,
    pub sale_price_minor: i64,
    pub purchase_price_minor: i64,
    pub tax_rate_id: i64,
    pub minimum_stock_milli: i64,
    pub allow_negative_stock: bool,
    pub active: bool,
    pub external_source: Option<ProductExternalSourceRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCategoryRequest {
    pub id: Option<i64>,
    pub name: String,
    #[serde(default = "default_true")]
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub id: i64,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxRateSummary {
    pub id: i64,
    pub name: String,
    pub rate_basis_points: i64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductExternalSource {
    pub provider: String,
    pub label: String,
    pub barcode: String,
    pub fetched_at: String,
    pub accepted_fields: Vec<String>,
}

pub type ProductExternalSourceRequest = ProductExternalSource;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductLookupFields {
    pub name: Option<String>,
    pub brand: Option<String>,
    pub image_url: Option<String>,
    pub package_size: Option<String>,
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductLookupSuggestion {
    pub barcode: String,
    pub source: ProductExternalSource,
    pub fields: ProductLookupFields,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSummary {
    pub id: i64,
    pub name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub unit_of_measure: String,
    pub sale_price_minor: i64,
    pub purchase_price_minor: i64,
    pub tax_rate_id: i64,
    pub tax_rate_name: String,
    pub tax_rate_basis_points: i64,
    pub minimum_stock_milli: i64,
    pub current_stock_milli: i64,
    pub allow_negative_stock: bool,
    pub active: bool,
    pub external_source: Option<ProductExternalSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductListResult {
    pub items: Vec<ProductSummary>,
    pub categories: Vec<CategorySummary>,
    pub tax_rates: Vec<TaxRateSummary>,
    pub total: usize,
}

struct NormalizedProductRequest {
    name: String,
    sku: String,
    barcode: Option<String>,
    category_id: Option<i64>,
    unit_of_measure: String,
    sale_price_minor: i64,
    purchase_price_minor: i64,
    tax_rate_id: i64,
    minimum_stock_milli: i64,
    allow_negative_stock: bool,
    active: bool,
    external_source: Option<ProductExternalSourceRequest>,
}

#[tauri::command]
pub fn catalog_search_products(
    state: State<'_, AppState>,
    query: ProductSearchQuery,
) -> Result<ProductListResult, CommandError> {
    search_products(state.db(), query).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_list_products(
    state: State<'_, AppState>,
    query: ProductListQuery,
) -> Result<ProductListResult, CommandError> {
    list_products(state.db(), query, DEFAULT_PRODUCT_LIST_LIMIT).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_get_product(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<ProductSummary>, CommandError> {
    get_product(state.db(), id).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_create_product(
    state: State<'_, AppState>,
    request: SaveProductRequest,
) -> Result<ProductSummary, CommandError> {
    super::auth::require_admin(state.inner())?;
    create_product(state.db(), request).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_update_product(
    state: State<'_, AppState>,
    id: i64,
    request: SaveProductRequest,
) -> Result<ProductSummary, CommandError> {
    super::auth::require_admin(state.inner())?;
    update_product(state.db(), id, request).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_set_product_active(
    state: State<'_, AppState>,
    id: i64,
    active: bool,
) -> Result<ProductSummary, CommandError> {
    super::auth::require_admin(state.inner())?;
    set_product_active(state.db(), id, active).map_err(Into::into)
}

#[tauri::command]
pub async fn catalog_lookup_product_by_barcode(
    barcode: String,
) -> Result<Option<ProductLookupSuggestion>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || lookup_product_by_barcode(&barcode))
        .await
        .map_err(|error| {
            CommandError::new(
                "external_lookup_failed",
                format!("Pretraga barcode-a nije uspela: {error}"),
            )
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub fn catalog_list_categories(
    state: State<'_, AppState>,
) -> Result<Vec<CategorySummary>, CommandError> {
    let connection = state.db().open()?;
    list_categories_for_connection(&connection).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_save_category(
    state: State<'_, AppState>,
    request: SaveCategoryRequest,
) -> Result<CategorySummary, CommandError> {
    super::auth::require_admin(state.inner())?;
    save_category(state.db(), request).map_err(Into::into)
}

pub fn search_products(db: &Db, query: ProductSearchQuery) -> Result<ProductListResult, AppError> {
    list_products(
        db,
        ProductListQuery {
            search: query.search,
            active: Some(query.active.unwrap_or(true)),
            ..ProductListQuery::default()
        },
        query.limit.unwrap_or(DEFAULT_PRODUCT_SEARCH_LIMIT),
    )
}

pub fn list_products(
    db: &Db,
    query: ProductListQuery,
    limit: i64,
) -> Result<ProductListResult, AppError> {
    let connection = db.open()?;
    list_products_for_connection(&connection, query, limit)
}

pub fn get_product(db: &Db, id: i64) -> Result<Option<ProductSummary>, AppError> {
    if id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "id"));
    }

    let connection = db.open()?;
    product_by_id_for_connection(&connection, id)
}

pub fn create_product(db: &Db, request: SaveProductRequest) -> Result<ProductSummary, AppError> {
    let mut connection = db.open()?;
    let normalized = normalize_product_request(request)?;
    let now = utc_now()?;
    let tx = connection.transaction()?;

    ensure_category_exists(&tx, normalized.category_id)?;
    ensure_active_tax_rate_exists(&tx, normalized.tax_rate_id)?;
    ensure_unique_product(&tx, None, &normalized.sku, normalized.barcode.as_deref())?;
    let external_source_accepted_fields_json =
        external_source_accepted_fields_json(normalized.external_source.as_ref())?;

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
            allow_negative_stock,
            active,
            external_source_provider,
            external_source_label,
            external_source_barcode,
            external_source_fetched_at,
            external_source_accepted_fields_json,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
        params![
            normalized.name,
            normalized.sku,
            normalized.barcode,
            normalized.category_id,
            normalized.unit_of_measure,
            normalized.sale_price_minor,
            normalized.purchase_price_minor,
            normalized.tax_rate_id,
            normalized.minimum_stock_milli,
            bool_to_i64(normalized.allow_negative_stock),
            bool_to_i64(normalized.active),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.provider.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.label.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.barcode.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.fetched_at.as_str()),
            external_source_accepted_fields_json,
            now,
        ],
    )?;
    let product_id = tx.last_insert_rowid();
    let product = product_by_id_for_connection(&tx, product_id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    tx.commit()?;
    Ok(product)
}

pub fn update_product(
    db: &Db,
    id: i64,
    request: SaveProductRequest,
) -> Result<ProductSummary, AppError> {
    if id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "id"));
    }

    let mut connection = db.open()?;
    let normalized = normalize_product_request(request)?;
    let now = utc_now()?;
    let tx = connection.transaction()?;

    ensure_product_exists(&tx, id)?;
    ensure_category_exists(&tx, normalized.category_id)?;
    ensure_active_tax_rate_exists(&tx, normalized.tax_rate_id)?;
    ensure_unique_product(
        &tx,
        Some(id),
        &normalized.sku,
        normalized.barcode.as_deref(),
    )?;
    let external_source_accepted_fields_json =
        external_source_accepted_fields_json(normalized.external_source.as_ref())?;

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
             allow_negative_stock = ?10,
             active = ?11,
             external_source_provider = ?12,
             external_source_label = ?13,
             external_source_barcode = ?14,
             external_source_fetched_at = ?15,
             external_source_accepted_fields_json = ?16,
             updated_at = ?17
         WHERE id = ?18",
        params![
            normalized.name,
            normalized.sku,
            normalized.barcode,
            normalized.category_id,
            normalized.unit_of_measure,
            normalized.sale_price_minor,
            normalized.purchase_price_minor,
            normalized.tax_rate_id,
            normalized.minimum_stock_milli,
            bool_to_i64(normalized.allow_negative_stock),
            bool_to_i64(normalized.active),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.provider.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.label.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.barcode.as_str()),
            normalized
                .external_source
                .as_ref()
                .map(|source| source.fetched_at.as_str()),
            external_source_accepted_fields_json,
            now,
            id,
        ],
    )?;

    let product = product_by_id_for_connection(&tx, id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    tx.commit()?;
    Ok(product)
}

pub fn set_product_active(db: &Db, id: i64, active: bool) -> Result<ProductSummary, AppError> {
    if id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "id"));
    }

    let mut connection = db.open()?;
    let now = utc_now()?;
    let tx = connection.transaction()?;
    let changed = tx.execute(
        "UPDATE products
         SET active = ?1, updated_at = ?2
         WHERE id = ?3",
        params![bool_to_i64(active), now, id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Artikal nije pronađen."));
    }

    let product = product_by_id_for_connection(&tx, id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    tx.commit()?;
    Ok(product)
}

pub fn save_category(db: &Db, request: SaveCategoryRequest) -> Result<CategorySummary, AppError> {
    let name = request.name.trim().to_string();

    if name.is_empty() {
        return Err(validation_error("Naziv kategorije je obavezan.", "name"));
    }

    if matches!(request.id, Some(id) if id <= 0) {
        return Err(validation_error("Kategorija nije ispravna.", "id"));
    }

    let mut connection = db.open()?;
    let now = utc_now()?;
    let tx = connection.transaction()?;

    ensure_unique_category(&tx, request.id, &name)?;

    let category_id = if let Some(id) = request.id {
        let changed = tx.execute(
            "UPDATE categories
             SET name = ?1,
                 active = ?2,
                 updated_at = ?3
             WHERE id = ?4",
            params![name, bool_to_i64(request.active), now, id],
        )?;

        if changed == 0 {
            return Err(AppError::not_found("Kategorija nije pronađena."));
        }

        id
    } else {
        tx.execute(
            "INSERT INTO categories (name, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![name, bool_to_i64(request.active), now],
        )?;
        tx.last_insert_rowid()
    };

    let category = category_by_id_for_connection(&tx, category_id)?
        .ok_or_else(|| AppError::not_found("Kategorija nije pronađena."))?;

    tx.commit()?;
    Ok(category)
}

pub fn lookup_product_by_barcode(
    barcode: &str,
) -> Result<Option<ProductLookupSuggestion>, AppError> {
    let barcode = normalize_lookup_barcode(barcode)?;
    let fetched_at = utc_now()?;
    let url = format!(
        "https://world.openfoodfacts.org/api/v3/product/{barcode}.json?fields=code,product_name,brands,quantity,categories,image_front_url"
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(OPEN_FOOD_FACTS_TIMEOUT_SECONDS))
        .build();
    let response = agent
        .get(&url)
        .set("User-Agent", OPEN_FOOD_FACTS_USER_AGENT)
        .call();

    match response {
        Ok(response) => {
            let body = response.into_string().map_err(|source| {
                AppError::InvalidState(format!("Odgovor izvora nije čitljiv: {source}"))
            })?;

            lookup_suggestion_from_open_food_facts_json(&barcode, &body, &fetched_at)
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(status, _)) => Err(AppError::business(
            "external_lookup_failed",
            format!("Open Food Facts nije vratio podatke za barcode ({status})."),
        )),
        Err(error) => Err(AppError::business(
            "external_lookup_failed",
            format!("Pretraga barcode-a nije dostupna: {error}"),
        )),
    }
}

pub fn lookup_suggestion_from_open_food_facts_json(
    barcode: &str,
    body: &str,
    fetched_at: &str,
) -> Result<Option<ProductLookupSuggestion>, AppError> {
    let root: serde_json::Value = serde_json::from_str(body).map_err(|source| {
        AppError::InvalidState(format!("Odgovor izvora nije ispravan JSON: {source}"))
    })?;
    let Some(product) = root.get("product") else {
        return Ok(None);
    };

    let fields = ProductLookupFields {
        name: json_string(product, "product_name"),
        brand: json_string(product, "brands"),
        image_url: json_string(product, "image_front_url"),
        package_size: json_string(product, "quantity"),
        category_name: json_string(product, "categories")
            .and_then(|categories| {
                categories
                    .rsplit(',')
                    .next()
                    .map(str::trim)
                    .map(str::to_string)
            })
            .filter(|category| !category.is_empty()),
    };

    if fields.name.is_none()
        && fields.brand.is_none()
        && fields.image_url.is_none()
        && fields.package_size.is_none()
        && fields.category_name.is_none()
    {
        return Ok(None);
    }

    Ok(Some(ProductLookupSuggestion {
        barcode: barcode.to_string(),
        source: ProductExternalSource {
            provider: "open_food_facts".to_string(),
            label: "Open Food Facts".to_string(),
            barcode: barcode.to_string(),
            fetched_at: fetched_at.to_string(),
            accepted_fields: Vec::new(),
        },
        fields,
    }))
}

pub fn list_products_for_connection(
    connection: &Connection,
    query: ProductListQuery,
    limit: i64,
) -> Result<ProductListResult, AppError> {
    let normalized_search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", crate::text::fold_text(value)));
    let active = query.active.map(bool_to_i64);
    let low_stock = bool_to_i64(query.low_stock.unwrap_or(false));
    let missing_barcode = bool_to_i64(query.missing_barcode.unwrap_or(false));
    let limit = limit.clamp(1, MAX_PRODUCT_LIST_LIMIT);

    let mut statement = connection.prepare(
        "SELECT
             p.id,
             p.name,
             p.sku,
             p.barcode,
             p.category_id,
             c.name AS category_name,
             p.unit_of_measure,
             p.sale_price_minor,
             p.purchase_price_minor,
             p.tax_rate_id,
             tr.name AS tax_rate_name,
             tr.rate_basis_points,
             p.minimum_stock_milli,
             COALESCE(ib.quantity_milli, 0) AS current_stock_milli,
             p.allow_negative_stock,
             p.active,
             p.external_source_provider,
             p.external_source_label,
             p.external_source_barcode,
             p.external_source_fetched_at,
             p.external_source_accepted_fields_json
         FROM products p
         JOIN tax_rates tr ON tr.id = p.tax_rate_id
         LEFT JOIN categories c ON c.id = p.category_id
         LEFT JOIN inventory_balances ib ON ib.product_id = p.id
         WHERE
             (?1 IS NULL
              OR fold(p.name) LIKE ?1
              OR fold(p.sku) LIKE ?1
              OR fold(COALESCE(p.barcode, '')) LIKE ?1)
             AND (?2 IS NULL OR p.active = ?2)
             AND (?3 IS NULL OR p.category_id = ?3)
             AND (?4 IS NULL OR p.tax_rate_id = ?4)
             AND (?5 = 0 OR (p.minimum_stock_milli > 0 AND COALESCE(ib.quantity_milli, 0) <= p.minimum_stock_milli))
             AND (?6 = 0 OR p.barcode IS NULL OR p.barcode = '')
         ORDER BY p.name ASC, p.id ASC
         LIMIT ?7",
    )?;
    let rows = statement.query_map(
        params![
            normalized_search,
            active,
            query.category_id,
            query.tax_rate_id,
            low_stock,
            missing_barcode,
            limit
        ],
        product_from_row,
    )?;
    let items = rows.collect::<Result<Vec<_>, _>>()?;
    let categories = list_categories_for_connection(connection)?;
    let tax_rates = list_tax_rates_for_connection(connection)?;
    let total = items.len();

    Ok(ProductListResult {
        items,
        categories,
        tax_rates,
        total,
    })
}

fn product_by_id_for_connection(
    connection: &Connection,
    id: i64,
) -> Result<Option<ProductSummary>, AppError> {
    connection
        .query_row(
            "SELECT
                 p.id,
                 p.name,
                 p.sku,
                 p.barcode,
                 p.category_id,
                 c.name AS category_name,
                 p.unit_of_measure,
                 p.sale_price_minor,
                 p.purchase_price_minor,
                 p.tax_rate_id,
                 tr.name AS tax_rate_name,
                 tr.rate_basis_points,
                 p.minimum_stock_milli,
                 COALESCE(ib.quantity_milli, 0) AS current_stock_milli,
                 p.allow_negative_stock,
                 p.active,
                 p.external_source_provider,
                 p.external_source_label,
                 p.external_source_barcode,
                 p.external_source_fetched_at,
                 p.external_source_accepted_fields_json
             FROM products p
             JOIN tax_rates tr ON tr.id = p.tax_rate_id
             LEFT JOIN categories c ON c.id = p.category_id
             LEFT JOIN inventory_balances ib ON ib.product_id = p.id
             WHERE p.id = ?1",
            params![id],
            product_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn list_categories_for_connection(
    connection: &Connection,
) -> Result<Vec<CategorySummary>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, name, active
         FROM categories
         ORDER BY name ASC",
    )?;
    let rows = statement.query_map([], category_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn list_tax_rates_for_connection(connection: &Connection) -> Result<Vec<TaxRateSummary>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, name, rate_basis_points, active
         FROM tax_rates
         ORDER BY rate_basis_points DESC, name ASC",
    )?;
    let rows = statement.query_map([], tax_rate_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn category_by_id_for_connection(
    connection: &Connection,
    id: i64,
) -> Result<Option<CategorySummary>, AppError> {
    connection
        .query_row(
            "SELECT id, name, active FROM categories WHERE id = ?1",
            params![id],
            category_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn ensure_product_exists(connection: &Connection, id: i64) -> Result<(), AppError> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM products WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_none() {
        return Err(AppError::not_found("Artikal nije pronađen."));
    }

    Ok(())
}

fn ensure_category_exists(
    connection: &Connection,
    category_id: Option<i64>,
) -> Result<(), AppError> {
    let Some(category_id) = category_id else {
        return Ok(());
    };

    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM categories WHERE id = ?1",
            params![category_id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_none() {
        return Err(validation_error("Kategorija nije pronađena.", "categoryId"));
    }

    Ok(())
}

fn ensure_active_tax_rate_exists(
    connection: &Connection,
    tax_rate_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM tax_rates WHERE id = ?1 AND active = 1",
            params![tax_rate_id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_none() {
        return Err(validation_error("Izaberite važeću PDV stopu.", "taxRateId"));
    }

    Ok(())
}

fn ensure_unique_product(
    connection: &Connection,
    current_product_id: Option<i64>,
    sku: &str,
    barcode: Option<&str>,
) -> Result<(), AppError> {
    let duplicate_sku: Option<i64> = connection
        .query_row(
            "SELECT id
             FROM products
             WHERE LOWER(sku) = LOWER(?1)
               AND (?2 IS NULL OR id <> ?2)
             LIMIT 1",
            params![sku, current_product_id],
            |row| row.get(0),
        )
        .optional()?;

    if duplicate_sku.is_some() {
        return Err(AppError::business(
            "duplicate_sku",
            "SKU/šifra već postoji.",
        ));
    }

    if let Some(barcode) = barcode {
        let duplicate_barcode: Option<i64> = connection
            .query_row(
                "SELECT id
                 FROM products
                 WHERE barcode = ?1
                   AND (?2 IS NULL OR id <> ?2)
                 LIMIT 1",
                params![barcode, current_product_id],
                |row| row.get(0),
            )
            .optional()?;

        if duplicate_barcode.is_some() {
            return Err(AppError::business(
                "duplicate_barcode",
                "Barcode već postoji.",
            ));
        }
    }

    Ok(())
}

fn ensure_unique_category(
    connection: &Connection,
    current_category_id: Option<i64>,
    name: &str,
) -> Result<(), AppError> {
    let duplicate: Option<i64> = connection
        .query_row(
            "SELECT id
             FROM categories
             WHERE LOWER(name) = LOWER(?1)
               AND (?2 IS NULL OR id <> ?2)
             LIMIT 1",
            params![name, current_category_id],
            |row| row.get(0),
        )
        .optional()?;

    if duplicate.is_some() {
        return Err(AppError::business(
            "duplicate_category",
            "Kategorija već postoji.",
        ));
    }

    Ok(())
}

fn normalize_product_request(
    request: SaveProductRequest,
) -> Result<NormalizedProductRequest, AppError> {
    let name = request.name.trim().to_string();
    let sku = request.sku.trim().to_string();
    let barcode = normalized_optional_text(request.barcode.as_deref());
    let unit_of_measure = request.unit_of_measure.trim().to_string();
    let external_source = normalize_external_source(request.external_source)?;

    if name.is_empty() {
        return Err(validation_error("Naziv je obavezan.", "name"));
    }

    if sku.is_empty() {
        return Err(validation_error("SKU/šifra je obavezna.", "sku"));
    }

    if unit_of_measure.is_empty() {
        return Err(validation_error(
            "Jedinica mere je obavezna.",
            "unitOfMeasure",
        ));
    }

    if matches!(request.category_id, Some(id) if id <= 0) {
        return Err(validation_error("Kategorija nije ispravna.", "categoryId"));
    }

    if request.sale_price_minor < 0 {
        return Err(validation_error(
            "Prodajna cena ne može biti negativna.",
            "salePriceMinor",
        ));
    }

    if request.purchase_price_minor < 0 {
        return Err(validation_error(
            "Nabavna cena ne može biti negativna.",
            "purchasePriceMinor",
        ));
    }

    if request.tax_rate_id <= 0 {
        return Err(validation_error("Izaberite PDV stopu.", "taxRateId"));
    }

    if request.minimum_stock_milli < 0 {
        return Err(validation_error(
            "Minimalni lager ne može biti negativan.",
            "minimumStockMilli",
        ));
    }

    Ok(NormalizedProductRequest {
        name,
        sku,
        barcode,
        category_id: request.category_id,
        unit_of_measure,
        sale_price_minor: request.sale_price_minor,
        purchase_price_minor: request.purchase_price_minor,
        tax_rate_id: request.tax_rate_id,
        minimum_stock_milli: request.minimum_stock_milli,
        allow_negative_stock: request.allow_negative_stock,
        active: request.active,
        external_source,
    })
}

fn product_from_row(row: &Row<'_>) -> rusqlite::Result<ProductSummary> {
    Ok(ProductSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        sku: row.get(2)?,
        barcode: row.get(3)?,
        category_id: row.get(4)?,
        category_name: row.get(5)?,
        unit_of_measure: row.get(6)?,
        sale_price_minor: row.get(7)?,
        purchase_price_minor: row.get(8)?,
        tax_rate_id: row.get(9)?,
        tax_rate_name: row.get(10)?,
        tax_rate_basis_points: row.get(11)?,
        minimum_stock_milli: row.get(12)?,
        current_stock_milli: row.get(13)?,
        allow_negative_stock: row.get::<_, i64>(14)? == 1,
        active: row.get::<_, i64>(15)? == 1,
        external_source: external_source_from_row(row, 16)?,
    })
}

fn category_from_row(row: &Row<'_>) -> rusqlite::Result<CategorySummary> {
    Ok(CategorySummary {
        id: row.get(0)?,
        name: row.get(1)?,
        active: row.get::<_, i64>(2)? == 1,
    })
}

fn tax_rate_from_row(row: &Row<'_>) -> rusqlite::Result<TaxRateSummary> {
    Ok(TaxRateSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        rate_basis_points: row.get(2)?,
        active: row.get::<_, i64>(3)? == 1,
    })
}

fn external_source_from_row(
    row: &Row<'_>,
    start: usize,
) -> rusqlite::Result<Option<ProductExternalSource>> {
    let provider: Option<String> = row.get(start)?;
    let Some(provider) = provider else {
        return Ok(None);
    };

    let accepted_fields_json: Option<String> = row.get(start + 4)?;
    let accepted_fields = accepted_fields_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
        .unwrap_or_default();

    Ok(Some(ProductExternalSource {
        provider,
        label: row.get(start + 1)?,
        barcode: row.get(start + 2)?,
        fetched_at: row.get(start + 3)?,
        accepted_fields,
    }))
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_external_source(
    source: Option<ProductExternalSourceRequest>,
) -> Result<Option<ProductExternalSourceRequest>, AppError> {
    let Some(source) = source else {
        return Ok(None);
    };

    let provider = source.provider.trim().to_string();
    let label = source.label.trim().to_string();
    let barcode = source.barcode.trim().to_string();
    let fetched_at = source.fetched_at.trim().to_string();
    let accepted_fields = source
        .accepted_fields
        .into_iter()
        .map(|field| field.trim().to_string())
        .filter(|field| !field.is_empty())
        .fold(Vec::<String>::new(), |mut fields, field| {
            if !fields.contains(&field) {
                fields.push(field);
            }
            fields
        });

    if provider.is_empty() || label.is_empty() || barcode.is_empty() || fetched_at.is_empty() {
        return Err(validation_error(
            "Izvor javnih podataka nije ispravan.",
            "externalSource",
        ));
    }

    Ok(Some(ProductExternalSourceRequest {
        provider,
        label,
        barcode,
        fetched_at,
        accepted_fields,
    }))
}

fn external_source_accepted_fields_json(
    source: Option<&ProductExternalSourceRequest>,
) -> Result<Option<String>, AppError> {
    source
        .map(|source| {
            serde_json::to_string(&source.accepted_fields).map_err(|error| {
                AppError::InvalidState(format!("Izvor javnih podataka nije sačuvan: {error}"))
            })
        })
        .transpose()
}

fn normalize_lookup_barcode(barcode: &str) -> Result<String, AppError> {
    let barcode = barcode.trim();

    if barcode.is_empty() {
        return Err(validation_error("Unesite barcode pre pretrage.", "barcode"));
    }

    if !barcode.chars().all(|character| character.is_ascii_digit()) {
        return Err(validation_error(
            "Barcode može da sadrži samo cifre.",
            "barcode",
        ));
    }

    Ok(barcode.to_string())
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn validation_error(message: &str, field: &str) -> AppError {
    AppError::validation(message, serde_json::json!({ "field": field }))
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tauri::Manager;

    use crate::app_error::CommandError;
    use crate::commands::catalog::{
        catalog_create_product, catalog_save_category, catalog_update_product, create_product,
        list_products, lookup_suggestion_from_open_food_facts_json, save_category, search_products,
        set_product_active, update_product, ProductExternalSourceRequest, ProductListQuery,
        ProductSearchQuery, SaveCategoryRequest, SaveProductRequest,
    };
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    fn with_catalog_database(test_name: &str, test: impl FnOnce(&Db)) {
        let db_path = test_database_path(test_name);

        {
            let db = Db::new(db_path.clone()).expect("database should initialize");
            seed_catalog(&db);
            test(&db);
        }

        std::fs::remove_file(&db_path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                db_path.display()
            )
        });
    }

    fn seed_catalog(db: &Db) {
        let connection = db.open().expect("database should open");
        connection
            .execute(
                "INSERT INTO categories (id, name, created_at, updated_at)
                 VALUES (1, 'Mlecni proizvodi', '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("category should insert");
        connection
            .execute(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        connection
            .execute(
                "INSERT INTO products (
                    id,
                    name,
                    sku,
                    barcode,
                    category_id,
                    unit_of_measure,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    active,
                    created_at,
                    updated_at
                 )
                 VALUES (1, 'Mleko 1 l', 'MLEKO-1L', '8600000000010', 1, 'kom', 15999, 12000, 1, 5000, 1, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("product should insert");
        connection
            .execute(
                "INSERT INTO products (
                    id,
                    name,
                    sku,
                    barcode,
                    category_id,
                    unit_of_measure,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    active,
                    created_at,
                    updated_at
                 )
                 VALUES (2, 'Arhiviran artikal', 'ARH-1', NULL, 1, 'kom', 1000, 700, 1, 0, 0, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("inactive product should insert");
        connection
            .execute(
                "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                 VALUES (1, 3000, '2026-06-18T10:00:00Z')",
                params![],
            )
            .expect("inventory balance should insert");
    }

    fn sign_in_cashier(state: &AppState) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('marko', 'Marko Markovic', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
    }

    fn product_request(sku: &str, barcode: Option<&str>) -> SaveProductRequest {
        SaveProductRequest {
            name: "Jogurt 1 l".to_string(),
            sku: sku.to_string(),
            barcode: barcode.map(ToString::to_string),
            category_id: Some(1),
            unit_of_measure: "kom".to_string(),
            sale_price_minor: 18000,
            purchase_price_minor: 11000,
            tax_rate_id: 1,
            minimum_stock_milli: 2000,
            allow_negative_stock: false,
            active: true,
            external_source: None,
        }
    }

    #[test]
    fn create_product_persists_catalog_fields() {
        with_catalog_database("create_product_persists_catalog_fields", |db| {
            let product = create_product(db, product_request("JOG-1L", Some("8600000000034")))
                .expect("product should create");

            assert_eq!(product.name, "Jogurt 1 l");
            assert_eq!(product.sku, "JOG-1L");
            assert_eq!(product.barcode.as_deref(), Some("8600000000034"));
            assert_eq!(product.category_name.as_deref(), Some("Mlecni proizvodi"));
            assert_eq!(product.tax_rate_name, "PDV 20");
            assert_eq!(product.current_stock_milli, 0);
            assert!(product.active);
        });
    }

    #[test]
    fn open_food_facts_response_maps_to_lookup_suggestion() {
        let suggestion = lookup_suggestion_from_open_food_facts_json(
            "4008400320328",
            r#"{
                "code": "4008400320328",
                "status": "success",
                "product": {
                    "product_name": "Kinder Bueno",
                    "brands": "Kinder",
                    "quantity": "43 g",
                    "categories": "Snacks, Chocolate bars",
                    "image_front_url": "https://images.openfoodfacts.org/kinder.jpg"
                }
            }"#,
            "2026-06-18T10:00:00Z",
        )
        .expect("lookup response should parse")
        .expect("lookup suggestion should exist");

        assert_eq!(suggestion.barcode, "4008400320328");
        assert_eq!(suggestion.source.provider, "open_food_facts");
        assert_eq!(suggestion.source.label, "Open Food Facts");
        assert_eq!(suggestion.fields.name.as_deref(), Some("Kinder Bueno"));
        assert_eq!(suggestion.fields.brand.as_deref(), Some("Kinder"));
        assert_eq!(suggestion.fields.package_size.as_deref(), Some("43 g"));
    }

    #[test]
    fn create_product_persists_external_source_provenance() {
        with_catalog_database("create_product_persists_external_source_provenance", |db| {
            let mut request = product_request("KINDER-BUENO", Some("4008400320328"));
            request.name = "Kinder Bueno".to_string();
            request.external_source = Some(ProductExternalSourceRequest {
                provider: "open_food_facts".to_string(),
                label: "Open Food Facts".to_string(),
                barcode: "4008400320328".to_string(),
                fetched_at: "2026-06-18T10:00:00Z".to_string(),
                accepted_fields: vec!["name".to_string(), "brand".to_string()],
            });

            let product = create_product(db, request).expect("product should create");
            let source = product
                .external_source
                .expect("source provenance should persist");

            assert_eq!(source.provider, "open_food_facts");
            assert_eq!(source.label, "Open Food Facts");
            assert_eq!(source.accepted_fields, vec!["name", "brand"]);
        });
    }

    #[test]
    fn create_product_rejects_duplicate_sku() {
        with_catalog_database("create_product_rejects_duplicate_sku", |db| {
            let error = create_product(db, product_request("mleko-1l", Some("8600000000034")))
                .expect_err("duplicate sku should fail");
            let command_error = CommandError::from(error);

            assert_eq!(command_error.code, "duplicate_sku");
        });
    }

    #[test]
    fn create_product_rejects_duplicate_barcode() {
        with_catalog_database("create_product_rejects_duplicate_barcode", |db| {
            let error = create_product(db, product_request("JOG-1L", Some("8600000000010")))
                .expect_err("duplicate barcode should fail");
            let command_error = CommandError::from(error);

            assert_eq!(command_error.code, "duplicate_barcode");
        });
    }

    #[test]
    fn search_products_folds_serbian_diacritics_and_case() {
        with_catalog_database("search_folds_serbian", |db| {
            db.open()
                .expect("database should open")
                .execute(
                    "INSERT INTO products (
                        name, sku, barcode, category_id, unit_of_measure, sale_price_minor,
                        purchase_price_minor, tax_rate_id, minimum_stock_milli, active,
                        created_at, updated_at)
                     VALUES ('KOŠULJA', 'KOS-1', '8600000000099', 1, 'kom', 250000, 180000, 1, 0, 1,
                        '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                    [],
                )
                .expect("kosulja should insert");

            for term in ["kosulja", "KOSULJA", "košulja", "KOŠULJA"] {
                let result = search_products(
                    db,
                    ProductSearchQuery {
                        search: Some(term.to_string()),
                        active: Some(true),
                        limit: Some(20),
                    },
                )
                .expect("search should succeed");
                assert!(
                    result.items.iter().any(|p| p.name == "KOŠULJA"),
                    "term {term} should find KOŠULJA"
                );
            }
        });
    }

    #[test]
    fn search_products_finds_active_product_by_barcode_sku_or_name() {
        with_catalog_database("search_products_finds_active_product", |db| {
            let result = search_products(
                db,
                ProductSearchQuery {
                    search: Some("8600000000010".to_string()),
                    active: Some(true),
                    limit: Some(20),
                },
            )
            .expect("search should succeed");

            assert_eq!(result.items.len(), 1);
            assert_eq!(result.items[0].name, "Mleko 1 l");
            assert_eq!(result.items[0].current_stock_milli, 3000);
        });
    }

    #[test]
    fn search_products_hides_inactive_products_by_default() {
        with_catalog_database("search_products_hides_inactive_products_by_default", |db| {
            let result = search_products(
                db,
                ProductSearchQuery {
                    search: Some("arhiviran".to_string()),
                    active: None,
                    limit: Some(20),
                },
            )
            .expect("search should succeed");

            assert!(result.items.is_empty());
        });
    }

    #[test]
    fn list_products_includes_current_stock() {
        with_catalog_database("list_products_includes_current_stock", |db| {
            let result = list_products(
                db,
                ProductListQuery {
                    search: Some("mleko".to_string()),
                    active: Some(true),
                    ..ProductListQuery::default()
                },
                50,
            )
            .expect("list should succeed");

            assert_eq!(result.items.len(), 1);
            assert_eq!(result.items[0].current_stock_milli, 3000);
            assert_eq!(result.categories[0].name, "Mlecni proizvodi");
            assert_eq!(result.tax_rates[0].name, "PDV 20");
        });
    }

    #[test]
    fn update_product_allows_existing_sku_and_updates_fields() {
        with_catalog_database(
            "update_product_allows_existing_sku_and_updates_fields",
            |db| {
                let mut request = product_request("MLEKO-1L", Some("8600000000010"));
                request.name = "Mleko 1.5 l".to_string();
                request.sale_price_minor = 21999;

                let updated = update_product(db, 1, request).expect("product should update");

                assert_eq!(updated.name, "Mleko 1.5 l");
                assert_eq!(updated.sale_price_minor, 21999);
                assert_eq!(updated.current_stock_milli, 3000);
            },
        );
    }

    #[test]
    fn set_product_active_updates_status() {
        with_catalog_database("set_product_active_updates_status", |db| {
            let updated = set_product_active(db, 1, false).expect("status should update");

            assert!(!updated.active);
        });
    }

    #[test]
    fn save_category_creates_and_updates_category() {
        with_catalog_database("save_category_creates_and_updates_category", |db| {
            let created = save_category(
                db,
                SaveCategoryRequest {
                    id: None,
                    name: "Pica".to_string(),
                    active: true,
                },
            )
            .expect("category should create");

            let updated = save_category(
                db,
                SaveCategoryRequest {
                    id: Some(created.id),
                    name: "Bezalkoholna pica".to_string(),
                    active: false,
                },
            )
            .expect("category should update");

            assert_eq!(updated.name, "Bezalkoholna pica");
            assert!(!updated.active);
        });
    }

    #[test]
    fn catalog_create_product_rejected_for_cashier() {
        let path = test_database_path("catalog_create_product_rejected_for_cashier");

        {
            let db = Db::new(path.clone()).expect("database should initialize");
            seed_catalog(&db);
            let state = AppState::new(db);
            sign_in_cashier(&state);

            // catalog_create_product takes a Tauri `State`, so a headless mock app
            // is needed to obtain a real managed state, mirroring the admin-gate
            // test pattern used in reports.rs/inventory.rs.
            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = catalog_create_product(
                app.state::<AppState>(),
                product_request("JOG-1L", Some("8600000000034")),
            )
            .expect_err("cashier should not create products");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }

    #[test]
    fn catalog_update_product_rejected_for_cashier() {
        let path = test_database_path("catalog_update_product_rejected_for_cashier");

        {
            let db = Db::new(path.clone()).expect("database should initialize");
            seed_catalog(&db);
            let state = AppState::new(db);
            sign_in_cashier(&state);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = catalog_update_product(
                app.state::<AppState>(),
                1,
                product_request("MLEKO-1L", Some("8600000000010")),
            )
            .expect_err("cashier should not update products");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }

    #[test]
    fn catalog_save_category_rejected_for_cashier() {
        let path = test_database_path("catalog_save_category_rejected_for_cashier");

        {
            let db = Db::new(path.clone()).expect("database should initialize");
            seed_catalog(&db);
            let state = AppState::new(db);
            sign_in_cashier(&state);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = catalog_save_category(
                app.state::<AppState>(),
                SaveCategoryRequest {
                    id: None,
                    name: "Pica".to_string(),
                    active: true,
                },
            )
            .expect_err("cashier should not save categories");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }
}
