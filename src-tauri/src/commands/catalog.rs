use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::commands::settings::{ShopProfile, SHOP_PROFILE_KEY};
use crate::db::Db;
use crate::price_history::{
    compute_prethodna_cena, load_offering_state, record_offered_price_change, IncomputableReason,
    OfferingState, PrethodnaCenaResult,
};
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
    /// Lako kvarljiva roba. Not a price field: it describes the goods, so a
    /// change here never enters `price_history`. It suppresses the čl. 37
    /// st. 3 computation instead — the log's prices for perishables reflect
    /// end-of-life markdowns, not the „najniža cena" a shopper compares
    /// against, so `campaigns.rs` demands a manual anchor for these.
    #[serde(default)]
    pub perishable: bool,
    #[serde(default)]
    pub perishable_justification: Option<String>,
    pub external_source: Option<ProductExternalSourceRequest>,
    /// ZoT čl. 34 st. 1 identity data. Nullable and `#[serde(default)]`: for
    /// walk-in retail the marking duty is the proizvođač's / uvoznik's (st. 2),
    /// so an operator who never fills these in must still be able to save.
    /// `validate_declaration` is what turns them into a requirement, and only
    /// for a shop that has answered „prodajem na daljinu" with yes.
    #[serde(default)]
    pub manufacturer_name: Option<String>,
    #[serde(default)]
    pub importer_name: Option<String>,
    #[serde(default)]
    pub country_of_origin: Option<String>,
    /// Reserved for the future jedinstveni šifarnik robe. Stored, never
    /// validated and never synced — the register does not exist yet.
    #[serde(default)]
    pub official_goods_code: Option<String>,
    /// `gtin` | `internal` | `none`. An in-house printed code must never be
    /// recorded as a GTIN, so the kind is asserted rather than guessed.
    #[serde(default)]
    pub barcode_kind: Option<String>,
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
    pub perishable: bool,
    pub perishable_justification: Option<String>,
    pub external_source: Option<ProductExternalSource>,
    pub manufacturer_name: Option<String>,
    pub importer_name: Option<String>,
    pub country_of_origin: Option<String>,
    pub official_goods_code: Option<String>,
    pub barcode_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductListResult {
    pub items: Vec<ProductSummary>,
    pub categories: Vec<CategorySummary>,
    pub tax_rates: Vec<TaxRateSummary>,
    pub total: usize,
}

/// One article whose deklaracija evidence in the catalog is incomplete.
///
/// Data only, and deliberately **no fine figure**: §4 item 11 leaves it
/// unresolved which penalty tier a bare missing GTIN falls under (fixed 40.000
/// vs 50.000–500.000 plus shop closure), and §3 req 26 forbids collapsing the
/// two. A blank column here is a gap in the shop's own records, not proof that
/// the goods on the shelf carry no deklaracija — the čl. 34 st. 1 data lives on
/// the packaging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationGapRow {
    pub product_id: i64,
    pub sku: String,
    pub name: String,
    pub barcode: Option<String>,
    pub barcode_kind: Option<String>,
    /// camelCase names of the blank ZoT čl. 34 st. 1 fields, so the UI can point
    /// at the very input that has to be filled in.
    pub missing_fields: Vec<String>,
    /// The article carries a barcode nobody has classified. `NULL` means
    /// „unclassified", never „is a GTIN" (§3 req 24).
    pub barcode_unclassified: bool,
    /// The barcode was *asserted* to be a GTIN and fails the modulo-10 check
    /// digit. Always `false` for `internal`, `none` and an unclassified code —
    /// an in-house printed code must never be reported as a broken GTIN.
    pub gtin_check_digit_invalid: bool,
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
    perishable: bool,
    perishable_justification: Option<String>,
    external_source: Option<ProductExternalSourceRequest>,
    manufacturer_name: Option<String>,
    importer_name: Option<String>,
    country_of_origin: Option<String>,
    official_goods_code: Option<String>,
    barcode_kind: Option<String>,
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
    let acting = super::auth::require_admin(state.inner())?;
    create_product(state.db(), request, acting.id).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_update_product(
    state: State<'_, AppState>,
    id: i64,
    request: SaveProductRequest,
) -> Result<ProductSummary, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    update_product(state.db(), id, request, acting.id).map_err(Into::into)
}

#[tauri::command]
pub fn catalog_set_product_active(
    state: State<'_, AppState>,
    id: i64,
    active: bool,
) -> Result<ProductSummary, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    set_product_active(state.db(), id, active, acting.id).map_err(Into::into)
}

/// „Artikli bez podataka deklaracije". Admin-gated: it enumerates the shop's own
/// compliance gaps and is a management view, not a till view.
#[tauri::command]
pub fn catalog_declaration_gaps(
    state: State<'_, AppState>,
) -> Result<Vec<DeclarationGapRow>, CommandError> {
    super::auth::require_admin(state.inner())?;
    declaration_gaps(state.db()).map_err(Into::into)
}

/// Flat DTO for the frontend. The domain uses a Rust enum; flattening here
/// keeps the TypeScript contract simple.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrethodnaCenaDto {
    /// "computed" | "incomputable"
    pub status: &'static str,
    pub price_minor: Option<i64>,
    pub window_days: Option<i64>,
    pub window_from: Option<String>,
    pub window_to: Option<String>,
    pub truncated: bool,
    /// "too_new_in_assortment" | "not_offered_in_window" | "no_history"
    pub reason: Option<&'static str>,
    pub age_days: Option<i64>,
}

impl From<PrethodnaCenaResult> for PrethodnaCenaDto {
    fn from(result: PrethodnaCenaResult) -> Self {
        match result {
            PrethodnaCenaResult::Computed(value) => Self {
                status: "computed",
                price_minor: Some(value.price_minor),
                window_days: Some(value.window_days),
                window_from: Some(value.window_from),
                window_to: Some(value.window_to),
                truncated: value.truncated,
                reason: None,
                age_days: None,
            },
            PrethodnaCenaResult::Incomputable(reason) => {
                let (reason_code, age_days) = match reason {
                    IncomputableReason::TooNewInAssortment { age_days } => {
                        ("too_new_in_assortment", Some(age_days))
                    }
                    IncomputableReason::NotOfferedInWindow => ("not_offered_in_window", None),
                    IncomputableReason::NoHistory => ("no_history", None),
                };
                Self {
                    status: "incomputable",
                    price_minor: None,
                    window_days: None,
                    window_from: None,
                    window_to: None,
                    truncated: false,
                    reason: Some(reason_code),
                    age_days,
                }
            }
        }
    }
}

pub fn prethodna_cena(
    db: &Db,
    product_id: i64,
    campaign_start: &str,
) -> Result<PrethodnaCenaDto, AppError> {
    let connection = db.open()?;
    Ok(compute_prethodna_cena(&connection, product_id, campaign_start)?.into())
}

#[tauri::command]
pub fn catalog_prethodna_cena(
    state: State<'_, AppState>,
    product_id: i64,
    campaign_start: String,
) -> Result<PrethodnaCenaDto, CommandError> {
    super::auth::require_session(state.inner())?;
    prethodna_cena(state.db(), product_id, &campaign_start).map_err(Into::into)
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

pub fn create_product(
    db: &Db,
    request: SaveProductRequest,
    acting_user_id: i64,
) -> Result<ProductSummary, AppError> {
    let mut connection = db.open()?;
    let normalized = normalize_product_request(request)?;
    validate_declaration(&load_shop_profile_for_connection(&connection)?, &normalized)?;
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
            perishable,
            perishable_justification,
            external_source_provider,
            external_source_label,
            external_source_barcode,
            external_source_fetched_at,
            external_source_accepted_fields_json,
            manufacturer_name,
            importer_name,
            country_of_origin,
            official_goods_code,
            barcode_kind,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?24)",
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
            bool_to_i64(normalized.perishable),
            normalized.perishable_justification,
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
            normalized.manufacturer_name,
            normalized.importer_name,
            normalized.country_of_origin,
            normalized.official_goods_code,
            normalized.barcode_kind,
            now,
        ],
    )?;
    let product_id = tx.last_insert_rowid();

    record_offered_price_change(
        &tx,
        product_id,
        None,
        OfferingState {
            active: normalized.active,
            price_minor: normalized.sale_price_minor,
        },
        "create",
        Some(acting_user_id),
        &now,
    )?;

    let product = product_by_id_for_connection(&tx, product_id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    tx.commit()?;
    Ok(product)
}

pub fn update_product(
    db: &Db,
    id: i64,
    request: SaveProductRequest,
    acting_user_id: i64,
) -> Result<ProductSummary, AppError> {
    if id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "id"));
    }

    let mut connection = db.open()?;
    let normalized = normalize_product_request(request)?;
    validate_declaration(&load_shop_profile_for_connection(&connection)?, &normalized)?;
    let now = utc_now()?;
    let tx = connection.transaction()?;

    ensure_product_exists(&tx, id)?;
    // Read the before-state BEFORE the UPDATE: afterwards it would compare the
    // new value to itself and record nothing.
    let before = load_offering_state(&tx, id)?;
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
             perishable = ?12,
             perishable_justification = ?13,
             external_source_provider = ?14,
             external_source_label = ?15,
             external_source_barcode = ?16,
             external_source_fetched_at = ?17,
             external_source_accepted_fields_json = ?18,
             manufacturer_name = ?19,
             importer_name = ?20,
             country_of_origin = ?21,
             official_goods_code = ?22,
             barcode_kind = ?23,
             updated_at = ?24
         WHERE id = ?25",
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
            bool_to_i64(normalized.perishable),
            normalized.perishable_justification,
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
            normalized.manufacturer_name,
            normalized.importer_name,
            normalized.country_of_origin,
            normalized.official_goods_code,
            normalized.barcode_kind,
            now,
            id,
        ],
    )?;

    record_offered_price_change(
        &tx,
        id,
        before,
        OfferingState {
            active: normalized.active,
            price_minor: normalized.sale_price_minor,
        },
        "update",
        Some(acting_user_id),
        &now,
    )?;

    let product = product_by_id_for_connection(&tx, id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    tx.commit()?;
    Ok(product)
}

pub fn set_product_active(
    db: &Db,
    id: i64,
    active: bool,
    acting_user_id: i64,
) -> Result<ProductSummary, AppError> {
    if id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "id"));
    }

    let mut connection = db.open()?;
    let now = utc_now()?;
    let tx = connection.transaction()?;
    // Read the before-state BEFORE the UPDATE: afterwards it would compare the
    // new value to itself and record nothing.
    let before = load_offering_state(&tx, id)?;
    let changed = tx.execute(
        "UPDATE products
         SET active = ?1, updated_at = ?2
         WHERE id = ?3",
        params![bool_to_i64(active), now, id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Artikal nije pronađen."));
    }

    if let Some(before_state) = before {
        record_offered_price_change(
            &tx,
            id,
            Some(before_state),
            OfferingState {
                active,
                price_minor: before_state.price_minor,
            },
            if active { "reactivate" } else { "deactivate" },
            Some(acting_user_id),
            &now,
        )?;
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
             p.external_source_accepted_fields_json,
             p.perishable,
             p.perishable_justification,
             p.manufacturer_name,
             p.importer_name,
             p.country_of_origin,
             p.official_goods_code,
             p.barcode_kind
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
                 p.external_source_accepted_fields_json,
                 p.perishable,
                 p.perishable_justification,
                 p.manufacturer_name,
                 p.importer_name,
                 p.country_of_origin,
                 p.official_goods_code,
                 p.barcode_kind
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
    let perishable_justification =
        normalized_optional_text(request.perishable_justification.as_deref());
    let manufacturer_name = normalized_optional_text(request.manufacturer_name.as_deref());
    let importer_name = normalized_optional_text(request.importer_name.as_deref());
    let country_of_origin = normalized_optional_text(request.country_of_origin.as_deref());
    let official_goods_code = normalized_optional_text(request.official_goods_code.as_deref());
    let barcode_kind = normalized_optional_text(request.barcode_kind.as_deref())
        .map(|kind| kind.to_ascii_lowercase());

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

    // Marking goods perishable switches off the čl. 37 st. 3 computation for
    // every campaign that touches them, so the reason has to be on the record
    // rather than in the head of whoever ticked the box.
    if request.perishable && perishable_justification.is_none() {
        return Err(validation_error(
            "Za lako kvarljivu robu unesite obrazloženje.",
            "perishableJustification",
        ));
    }

    if let Some(kind) = barcode_kind.as_deref() {
        if !matches!(kind, "gtin" | "internal" | "none") {
            return Err(validation_error(
                "Vrsta barkoda mora biti GTIN, interni ili bez barkoda.",
                "barcodeKind",
            ));
        }
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
        perishable: request.perishable,
        // A justification without the flag is dead text: drop it so the two
        // columns can never disagree about whether the goods are perishable.
        perishable_justification: request
            .perishable
            .then_some(perishable_justification)
            .flatten(),
        external_source,
        manufacturer_name,
        importer_name,
        country_of_origin,
        official_goods_code,
        barcode_kind,
    })
}

/// ZoT čl. 34 st. 5: in daljinska trgovina the trgovac must himself display the
/// deklaracija and keep the st. 1 data continuously available before purchase.
/// For walk-in retail the marking duty is the proizvođač's/uvoznik's (st. 2),
/// so these fields are an aid, not a record — do not require them.
///
/// Only an explicit `Some(true)` blocks. `None` means the shop has not been
/// asked yet (§5 Q-8) and must be treated like walk-in retail: §4 item 12
/// forbids a hard block there, and inferring "yes" from silence would strand
/// legitimate stock just as inferring "no" would hide a real duty.
fn validate_declaration(
    profile: &ShopProfile,
    input: &NormalizedProductRequest,
) -> Result<(), AppError> {
    if profile.distance_selling != Some(true) {
        return Ok(());
    }

    // The finished sentence travels with the field: „zemlja" is feminine and
    // „ime" is neuter, so a shared `format!("{label} je obavezno …")` would put
    // ungrammatical Serbian in front of the operator for one of the two.
    for (value, field, message) in [
        (
            &input.manufacturer_name,
            "manufacturerName",
            "Poslovno ime proizvođača je obavezno za prodaju na daljinu.",
        ),
        (
            &input.country_of_origin,
            "countryOfOrigin",
            "Zemlja proizvodnje je obavezna za prodaju na daljinu.",
        ),
    ] {
        if value.as_deref().map(str::trim).unwrap_or("").is_empty() {
            return Err(AppError::validation(
                message,
                serde_json::json!({ "field": field }),
            ));
        }
    }

    Ok(())
}

/// The profile is one JSON blob in `settings`, and `load_shop_profile` takes the
/// `AppState` the catalog writers do not hold — they take a `&Db`. Read it off
/// the connection that is already open instead of widening every signature.
///
/// A missing row is the honest default — the operator has simply not answered
/// §5 Q-8 yet. A row that will *not* deserialize is a different thing, and it
/// fails loudly with the same „Podešavanja nisu ispravna" that
/// `settings::load_json_setting` raises for the identical row: degrading to
/// `ShopProfile::default()` would silently pick the lenient §3 req 22 branch and
/// switch the [LEGAL] čl. 34 st. 5 gate off with no signal anywhere.
pub(crate) fn load_shop_profile_for_connection(
    connection: &Connection,
) -> Result<ShopProfile, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![SHOP_PROFILE_KEY],
            |row| row.get(0),
        )
        .optional()?;

    match stored {
        Some(value) => serde_json::from_str(&value).map_err(|source| {
            AppError::InvalidState(format!("Podešavanja nisu ispravna: {source}"))
        }),
        None => Ok(ShopProfile::default()),
    }
}

/// GTIN-8/12/13/14 modulo-10 check digit. Runs ONLY for `barcode_kind = 'gtin'`
/// — an in-house printed code must never be validated as, or reported as, a GTIN
/// (§3 req 24).
pub fn is_valid_gtin(code: &str) -> bool {
    if !matches!(code.len(), 8 | 12 | 13 | 14) || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let digits: Vec<u32> = code.bytes().map(|b| u32::from(b - b'0')).collect();
    let (body, check) = digits.split_at(digits.len() - 1);
    // Weights run 3,1,3,1,… from the rightmost body digit, which is the same
    // rule for every GTIN length.
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| if index % 2 == 0 { digit * 3 } else { *digit })
        .sum();
    (10 - (sum % 10)) % 10 == check[0]
}

/// The articles whose deklaracija evidence is incomplete: a blank čl. 34 st. 1
/// identity field, a barcode nobody has classified, or a code asserted to be a
/// GTIN that fails its check digit.
///
/// Active articles only. Čl. 68 st. 1 tač. 9 punishes *selling* goods without a
/// deklaracija, so an archived article is not a gap the shop can act on, and
/// padding the list with dead rows would bury the ones that matter.
///
/// Never blocks anything and carries no fine figure — see `DeclarationGapRow`.
pub fn declaration_gaps(db: &Db) -> Result<Vec<DeclarationGapRow>, AppError> {
    let connection = db.open()?;
    let mut statement = connection.prepare(
        r#"
SELECT
    p.id,
    p.sku,
    p.name,
    p.barcode,
    p.barcode_kind,
    p.manufacturer_name,
    p.country_of_origin
FROM products p
WHERE p.active = 1
ORDER BY p.name, p.id
"#,
    )?;

    let rows = statement
        .query_map([], |row| {
            let barcode: Option<String> = row.get(3)?;
            let barcode_kind: Option<String> = row.get(4)?;
            let manufacturer_name: Option<String> = row.get(5)?;
            let country_of_origin: Option<String> = row.get(6)?;

            // The same two fields the čl. 34 st. 5 gate and the goods-receipt
            // warning use, so the three surfaces can never disagree about what
            // „nedostaje" means.
            let missing_fields = [
                (manufacturer_name, "manufacturerName"),
                (country_of_origin, "countryOfOrigin"),
            ]
            .into_iter()
            .filter(|(value, _)| value.as_deref().map(str::trim).unwrap_or("").is_empty())
            .map(|(_, field)| field.to_string())
            .collect::<Vec<_>>();

            let barcode_unclassified = barcode.is_some() && barcode_kind.is_none();
            let gtin_check_digit_invalid = barcode_kind.as_deref() == Some("gtin")
                && barcode.as_deref().is_some_and(|code| !is_valid_gtin(code));

            Ok(DeclarationGapRow {
                product_id: row.get(0)?,
                sku: row.get(1)?,
                name: row.get(2)?,
                barcode,
                barcode_kind,
                missing_fields,
                barcode_unclassified,
                gtin_check_digit_invalid,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows
        .into_iter()
        .filter(|row| {
            !row.missing_fields.is_empty()
                || row.barcode_unclassified
                || row.gtin_check_digit_invalid
        })
        .collect())
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
        perishable: row.get::<_, i64>(21)? == 1,
        perishable_justification: row.get(22)?,
        manufacturer_name: row.get(23)?,
        importer_name: row.get(24)?,
        country_of_origin: row.get(25)?,
        official_goods_code: row.get(26)?,
        barcode_kind: row.get(27)?,
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

    use crate::app_error::{AppError, CommandError};
    use crate::commands::catalog::{
        catalog_create_product, catalog_declaration_gaps, catalog_save_category,
        catalog_update_product, create_product, declaration_gaps, get_product, is_valid_gtin,
        list_products, lookup_suggestion_from_open_food_facts_json, prethodna_cena, save_category,
        search_products, set_product_active, update_product, ProductExternalSourceRequest,
        ProductListQuery, ProductSearchQuery, ProductSummary, SaveCategoryRequest,
        SaveProductRequest,
    };
    use crate::commands::settings::{
        save_shop_profile, PravnaForma, ShopProfileRequest, SHOP_PROFILE_KEY,
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
            perishable: false,
            perishable_justification: None,
            external_source: None,
            manufacturer_name: None,
            importer_name: None,
            country_of_origin: None,
            official_goods_code: None,
            barcode_kind: None,
        }
    }

    /// The declaration gate reads `shop_profile` out of `settings`, and
    /// `save_shop_profile` is admin-gated, so these tests need a real
    /// `AppState` with a session rather than the bare `Db` the older catalog
    /// tests use.
    fn with_catalog_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let db_path = test_database_path(test_name);

        {
            let db = Db::new(db_path.clone()).expect("database should initialize");
            seed_catalog(&db);
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&db_path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                db_path.display()
            )
        });
    }

    fn sign_in_admin(state: &AppState) {
        state
            .set_session_user_id(admin_id(state.db()))
            .expect("admin session should set");
    }

    fn create_product_as_admin(
        state: &AppState,
        request: SaveProductRequest,
    ) -> Result<ProductSummary, AppError> {
        let acting = admin_id(state.db());
        create_product(state.db(), request, acting)
    }

    fn update_product_as_admin(
        state: &AppState,
        id: i64,
        request: SaveProductRequest,
    ) -> Result<ProductSummary, AppError> {
        let acting = admin_id(state.db());
        update_product(state.db(), id, request, acting)
    }

    fn walk_in_profile_request() -> ShopProfileRequest {
        ShopProfileRequest {
            pravna_forma: Some(PravnaForma::Preduzetnik),
            pdv_obveznik: Some(false),
            distance_selling: Some(false),
            lpfr_in_premises: Some(true),
            esir_elements: Vec::new(),
        }
    }

    fn distance_selling_profile_request() -> ShopProfileRequest {
        ShopProfileRequest {
            distance_selling: Some(true),
            ..walk_in_profile_request()
        }
    }

    fn product_request_without_declaration(sku: &str) -> SaveProductRequest {
        product_request(sku, None)
    }

    fn product_request_with_declaration(sku: &str) -> SaveProductRequest {
        SaveProductRequest {
            manufacturer_name: Some("Mlekara Šabac d.o.o.".to_string()),
            importer_name: Some("Uvoznik Beograd d.o.o.".to_string()),
            country_of_origin: Some("Srbija".to_string()),
            official_goods_code: Some("SIF-0001".to_string()),
            barcode_kind: Some("internal".to_string()),
            ..product_request(sku, None)
        }
    }

    #[test]
    fn declaration_fields_are_advisory_for_walk_in_retail() {
        with_catalog_state("declaration_advisory", |state| {
            sign_in_admin(state);
            save_shop_profile(state, walk_in_profile_request()).expect("profile saves");
            // distance_selling: Some(false) — the shop was asked and said no

            let product =
                create_product_as_admin(state, product_request_without_declaration("DEK-WALKIN"))
                    .expect("a walk-in shop can save a product with no declaration data");

            assert_eq!(product.manufacturer_name, None);
        });
    }

    #[test]
    fn declaration_fields_never_block_while_distance_selling_is_unanswered() {
        with_catalog_state("declaration_unanswered", |state| {
            sign_in_admin(state);
            // distance_selling stays None — the profile was never filled in

            create_product_as_admin(state, product_request_without_declaration("DEK-UNANSWERED"))
                .expect(
                    "an unanswered profile must not block: §4 item 12 forbids a walk-in block, \
                     and silence is not an assertion that the shop sells at distance",
                );
        });
    }

    #[test]
    fn declaration_fields_are_required_once_the_shop_sells_at_distance() {
        with_catalog_state("declaration_required_at_distance", |state| {
            sign_in_admin(state);
            save_shop_profile(state, distance_selling_profile_request()).expect("profile saves");

            let error =
                create_product_as_admin(state, product_request_without_declaration("DEK-DISTANCE"))
                    .expect_err(
                        "cl. 34 st. 5 requires the st. 1 data to be available pre-purchase",
                    );

            assert!(matches!(error, AppError::Validation { .. }));
            let AppError::Validation { details, .. } = error else {
                unreachable!()
            };
            assert_eq!(
                details.expect("field details")["field"],
                serde_json::json!("manufacturerName")
            );
        });
    }

    /// „Zemlja" is feminine, so „obavezno" is wrong for it. The message is the
    /// only thing the operator ever sees of this gate; ungrammatical Serbian on
    /// a [LEGAL] block reads as a machine error rather than a duty.
    #[test]
    fn declaration_messages_agree_with_the_gender_of_the_field() {
        with_catalog_state("declaration_message_gender", |state| {
            sign_in_admin(state);
            save_shop_profile(state, distance_selling_profile_request()).expect("profile saves");

            let mut request = product_request_with_declaration("DEK-GENDER");
            request.country_of_origin = None;

            let error = create_product_as_admin(state, request)
                .expect_err("a missing country of production must block a distance seller");

            let AppError::Validation { message, details } = error else {
                panic!("expected a validation error")
            };
            assert_eq!(
                message,
                "Zemlja proizvodnje je obavezna za prodaju na daljinu."
            );
            assert_eq!(
                details.expect("field details")["field"],
                serde_json::json!("countryOfOrigin")
            );

            let mut missing_manufacturer = product_request_with_declaration("DEK-GENDER-2");
            missing_manufacturer.manufacturer_name = None;

            let error = create_product_as_admin(state, missing_manufacturer)
                .expect_err("a missing manufacturer must block a distance seller");

            let AppError::Validation { message, .. } = error else {
                panic!("expected a validation error")
            };
            assert_eq!(
                message,
                "Poslovno ime proizvođača je obavezno za prodaju na daljinu."
            );
        });
    }

    /// A `shop_profile` row that will not deserialize must not silently pick the
    /// lenient §3 req 22 branch. `settings::load_json_setting` already raises
    /// „Podešavanja nisu ispravna" for exactly this row, so the catalogue has to
    /// agree with it — otherwise the settings screen errors while the [LEGAL]
    /// čl. 34 st. 5 gate quietly stops firing.
    #[test]
    fn a_corrupt_shop_profile_is_loud_rather_than_lenient() {
        with_catalog_state("declaration_corrupt_profile", |state| {
            sign_in_admin(state);
            save_shop_profile(state, distance_selling_profile_request()).expect("profile saves");

            state
                .db()
                .open()
                .expect("db open")
                .execute(
                    "UPDATE settings SET value_json = ?2 WHERE key = ?1",
                    params![SHOP_PROFILE_KEY, "{ this is not json"],
                )
                .expect("the row should be overwritten");

            let error =
                create_product_as_admin(state, product_request_without_declaration("DEK-CORRUPT"))
                    .expect_err("an unreadable profile must not pass the gate by default");

            let AppError::InvalidState(message) = error else {
                panic!("expected an invalid-state error, not a silent default profile")
            };
            assert!(
                message.starts_with("Podešavanja nisu ispravna"),
                "message should match the canonical loader: {message}"
            );
        });
    }

    #[test]
    fn declaration_gate_also_guards_the_update_path() {
        with_catalog_state("declaration_required_on_update", |state| {
            sign_in_admin(state);

            // Saved while the shop still traded only over the counter.
            let product =
                create_product_as_admin(state, product_request_without_declaration("DEK-UPDATE"))
                    .expect("walk-in save is allowed");

            save_shop_profile(state, distance_selling_profile_request()).expect("profile saves");

            let error = update_product_as_admin(
                state,
                product.id,
                product_request_without_declaration("DEK-UPDATE"),
            )
            .expect_err("editing an article for a distance seller must demand the st. 1 data");

            assert!(matches!(error, AppError::Validation { .. }));
        });
    }

    #[test]
    fn country_of_origin_accepts_the_literal_eu() {
        with_catalog_state("declaration_accepts_eu", |state| {
            sign_in_admin(state);
            save_shop_profile(state, distance_selling_profile_request()).expect("profile saves");

            let mut request = product_request_with_declaration("DEK-EU");
            request.country_of_origin = Some("EU".to_string());

            create_product_as_admin(state, request)
                .expect("ZoT cl. 34 st. 7 permits the literal 'EU'");
        });
    }

    #[test]
    fn declaration_fields_round_trip_through_create_update_and_read() {
        with_catalog_state("declaration_round_trip", |state| {
            sign_in_admin(state);

            let created =
                create_product_as_admin(state, product_request_with_declaration("DEK-RT"))
                    .expect("product should create");

            assert_eq!(
                created.manufacturer_name.as_deref(),
                Some("Mlekara Šabac d.o.o.")
            );
            assert_eq!(
                created.importer_name.as_deref(),
                Some("Uvoznik Beograd d.o.o.")
            );
            assert_eq!(created.country_of_origin.as_deref(), Some("Srbija"));
            assert_eq!(created.official_goods_code.as_deref(), Some("SIF-0001"));
            assert_eq!(created.barcode_kind.as_deref(), Some("internal"));

            let reloaded = get_product(state.db(), created.id)
                .expect("product should read")
                .expect("product exists");
            assert_eq!(reloaded.country_of_origin.as_deref(), Some("Srbija"));

            let mut edited = product_request_with_declaration("DEK-RT");
            edited.country_of_origin = Some("Nemačka".to_string());
            let updated =
                update_product_as_admin(state, created.id, edited).expect("product should update");
            assert_eq!(updated.country_of_origin.as_deref(), Some("Nemačka"));
        });
    }

    #[test]
    fn declaration_barcode_kind_rejects_a_value_the_column_cannot_hold() {
        with_catalog_state("declaration_barcode_kind_guard", |state| {
            sign_in_admin(state);

            let mut request = product_request_without_declaration("DEK-KIND");
            request.barcode_kind = Some("qr".to_string());

            let error = create_product_as_admin(state, request)
                .expect_err("only gtin/internal/none may reach the column's CHECK");

            assert!(matches!(error, AppError::Validation { .. }));
        });
    }

    fn admin_id(db: &Db) -> i64 {
        db.open()
            .expect("db open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist")
    }

    fn price_rows(db: &Db, product_id: i64) -> Vec<(Option<i64>, String)> {
        let connection = db.open().expect("db open");
        let mut stmt = connection
            .prepare(
                "SELECT price_minor, source FROM price_history WHERE product_id = ?1 ORDER BY id",
            )
            .expect("prepare");
        let mapped = stmt
            .query_map(params![product_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        mapped.collect::<Result<Vec<_>, _>>().expect("collect")
    }

    #[test]
    fn creating_an_active_product_appends_a_create_price_row() {
        with_catalog_database("price_history_create_row", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-1", None), acting)
                .expect("product should create");

            assert_eq!(
                price_rows(db, product.id),
                vec![(Some(18000), "create".to_string())]
            );
        });
    }

    #[test]
    fn creating_an_inactive_product_appends_no_price_row() {
        with_catalog_database("price_history_create_inactive", |db| {
            let acting = admin_id(db);
            let mut request = product_request("PH-INACTIVE", None);
            request.active = false;
            let product = create_product(db, request, acting).expect("product should create");

            assert!(
                price_rows(db, product.id).is_empty(),
                "an unoffered product has no offered price to log"
            );
        });
    }

    #[test]
    fn updating_only_the_name_appends_no_price_row() {
        with_catalog_database("price_history_name_only", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-2", None), acting)
                .expect("product should create");

            let mut renamed = product_request("PH-2", None);
            renamed.name = "Novo ime".to_string();
            update_product(db, product.id, renamed, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id).len(),
                1,
                "editing the name must not pollute the price timeline"
            );
        });
    }

    #[test]
    fn lowering_the_price_appends_an_update_price_row() {
        with_catalog_database("price_history_price_change", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-3", None), acting)
                .expect("product should create");

            let mut cheaper = product_request("PH-3", None);
            cheaper.sale_price_minor = 15000;
            update_product(db, product.id, cheaper, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id),
                vec![
                    (Some(18000), "create".to_string()),
                    (Some(15000), "update".to_string())
                ]
            );
        });
    }

    #[test]
    fn deactivating_then_reactivating_records_a_gap_then_a_return() {
        with_catalog_database("price_history_gap_then_return", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-4", None), acting)
                .expect("product should create");

            set_product_active(db, product.id, false, acting).expect("deactivate");
            set_product_active(db, product.id, true, acting).expect("reactivate");

            assert_eq!(
                price_rows(db, product.id),
                vec![
                    (Some(18000), "create".to_string()),
                    (None, "deactivate".to_string()),
                    (Some(18000), "reactivate".to_string()),
                ]
            );
        });
    }

    #[test]
    fn setting_active_to_its_current_value_appends_nothing() {
        with_catalog_database("price_history_active_noop", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-5", None), acting)
                .expect("product should create");

            set_product_active(db, product.id, true, acting).expect("no-op activate");

            assert_eq!(
                price_rows(db, product.id).len(),
                1,
                "no state change, no row"
            );
        });
    }

    #[test]
    fn deactivating_via_update_product_also_records_the_gap() {
        with_catalog_database("price_history_update_deactivates", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-6", None), acting)
                .expect("product should create");

            // update_product sets `active` too, so it can end an offering.
            let mut deactivated = product_request("PH-6", None);
            deactivated.active = false;
            update_product(db, product.id, deactivated, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id),
                vec![
                    (Some(18000), "create".to_string()),
                    (None, "update".to_string())
                ]
            );
        });
    }

    #[test]
    fn price_row_is_attributed_to_the_acting_user() {
        with_catalog_database("price_history_attribution", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-7", None), acting)
                .expect("product should create");

            let user_id: Option<i64> = db
                .open()
                .expect("db open")
                .query_row(
                    "SELECT user_id FROM price_history WHERE product_id = ?1 ORDER BY id LIMIT 1",
                    params![product.id],
                    |row| row.get(0),
                )
                .expect("query");

            assert_eq!(user_id, Some(acting));
        });
    }

    #[test]
    fn create_product_persists_catalog_fields() {
        with_catalog_database("create_product_persists_catalog_fields", |db| {
            let product = create_product(
                db,
                product_request("JOG-1L", Some("8600000000034")),
                admin_id(db),
            )
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

            let product = create_product(db, request, admin_id(db)).expect("product should create");
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
            let error = create_product(
                db,
                product_request("mleko-1l", Some("8600000000034")),
                admin_id(db),
            )
            .expect_err("duplicate sku should fail");
            let command_error = CommandError::from(error);

            assert_eq!(command_error.code, "duplicate_sku");
        });
    }

    #[test]
    fn create_product_rejects_duplicate_barcode() {
        with_catalog_database("create_product_rejects_duplicate_barcode", |db| {
            let error = create_product(
                db,
                product_request("JOG-1L", Some("8600000000010")),
                admin_id(db),
            )
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

                let updated =
                    update_product(db, 1, request, admin_id(db)).expect("product should update");

                assert_eq!(updated.name, "Mleko 1.5 l");
                assert_eq!(updated.sale_price_minor, 21999);
                assert_eq!(updated.current_stock_milli, 3000);
            },
        );
    }

    #[test]
    fn create_product_persists_perishable_flag_and_justification() {
        with_catalog_database(
            "create_product_persists_perishable_flag_and_justification",
            |db| {
                let mut request = product_request("JOG-1L", Some("8600000000034"));
                request.perishable = true;
                request.perishable_justification =
                    Some("  Rok trajanja 7 dana — cena pada pred istek.  ".to_string());

                let product =
                    create_product(db, request, admin_id(db)).expect("product should create");

                assert!(product.perishable);
                assert_eq!(
                    product.perishable_justification.as_deref(),
                    Some("Rok trajanja 7 dana — cena pada pred istek."),
                    "justification is trimmed on the way in"
                );

                let reread = get_product(db, product.id)
                    .expect("product should read")
                    .expect("product should exist");
                assert!(reread.perishable);
                assert_eq!(
                    reread.perishable_justification.as_deref(),
                    Some("Rok trajanja 7 dana — cena pada pred istek.")
                );
            },
        );
    }

    #[test]
    fn create_product_rejects_perishable_without_justification() {
        with_catalog_database(
            "create_product_rejects_perishable_without_justification",
            |db| {
                let mut request = product_request("JOG-1L", Some("8600000000034"));
                request.perishable = true;
                request.perishable_justification = Some("   ".to_string());

                let error = create_product(db, request, admin_id(db))
                    .expect_err("perishable without justification should be rejected");

                match error {
                    AppError::Validation { message, details } => {
                        assert_eq!(message, "Za lako kvarljivu robu unesite obrazloženje.");
                        assert_eq!(
                            details.expect("validation error should name the field")["field"],
                            "perishableJustification"
                        );
                    }
                    other => panic!("expected a validation error, got {other:?}"),
                }
            },
        );
    }

    /// The perishable flag describes the goods, not the offer. Flipping it
    /// must not enter `price_history` — a čl. 37 st. 3 window is built from
    /// offered PRICES, and a spurious row there would move a shopper-facing
    /// prethodna cena for a change no shopper ever saw.
    #[test]
    fn perishable_flag_change_is_not_a_price_event() {
        with_catalog_database("perishable_flag_change_is_not_a_price_event", |db| {
            let created = create_product(
                db,
                product_request("JOG-1L", Some("8600000000034")),
                admin_id(db),
            )
            .expect("product should create");
            let before = price_rows(db, created.id);

            let mut request = product_request("JOG-1L", Some("8600000000034"));
            request.perishable = true;
            request.perishable_justification = Some("Kratak rok trajanja.".to_string());
            let updated = update_product(db, created.id, request, admin_id(db))
                .expect("product should update");

            assert!(updated.perishable);
            assert_eq!(
                price_rows(db, created.id),
                before,
                "a perishable flag change is not an offered-price change"
            );
        });
    }

    #[test]
    fn set_product_active_updates_status() {
        with_catalog_database("set_product_active_updates_status", |db| {
            let updated =
                set_product_active(db, 1, false, admin_id(db)).expect("status should update");

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

    #[test]
    fn prethodna_cena_dto_reports_computed_values() {
        with_catalog_database("prethodna_cena_dto_computed", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PC-1", None), acting)
                .expect("product should create");

            let connection = db.open().expect("db open");
            // Old enough in assortment for the full 30-day window (st. 3).
            connection
                .execute(
                    "UPDATE products SET created_at = '2026-04-01T00:00:00Z' WHERE id = ?1",
                    params![product.id],
                )
                .expect("created_at should back-date");
            // Replace the create row with the worked-example timeline.
            connection
                .execute(
                    "DELETE FROM price_history WHERE product_id = ?1",
                    params![product.id],
                )
                .expect("clear");
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     VALUES (?1, '2026-04-01T00:00:00Z', 499000, 'create', '2026-04-01T00:00:00Z'),
                            (?1, '2026-07-01T00:00:00Z', 529000, 'update', '2026-07-01T00:00:00Z')",
                    params![product.id],
                )
                .expect("timeline should insert");
            drop(connection);

            let dto = prethodna_cena(db, product.id, "2026-07-20T00:00:00Z").expect("compute");

            assert_eq!(dto.status, "computed");
            assert_eq!(dto.price_minor, Some(499000));
            assert_eq!(dto.window_days, Some(30));
            assert_eq!(dto.reason, None);
        });
    }

    #[test]
    fn prethodna_cena_dto_reports_too_new_in_assortment() {
        with_catalog_database("prethodna_cena_dto_too_new", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PC-2", None), acting)
                .expect("product should create");

            db.open()
                .expect("db open")
                .execute(
                    "UPDATE products SET created_at = '2026-07-11T00:00:00Z' WHERE id = ?1",
                    params![product.id],
                )
                .expect("created_at should back-date");

            let dto = prethodna_cena(db, product.id, "2026-07-17T00:00:00Z").expect("compute");

            assert_eq!(dto.status, "incomputable");
            assert_eq!(dto.reason, Some("too_new_in_assortment"));
            assert_eq!(dto.age_days, Some(6));
            assert_eq!(dto.price_minor, None);
        });
    }

    /// A barcode nobody has classified, and no čl. 34 st. 1 data at all.
    fn seed_product_without_declaration(state: &AppState) -> i64 {
        create_product_as_admin(
            state,
            SaveProductRequest {
                barcode: Some("8600000000027".to_string()),
                ..product_request_without_declaration("DEK-GAP-MISSING")
            },
        )
        .expect("product without declaration data should save")
        .id
    }

    /// Nothing to report: the identity fields are filled in and the barcode is
    /// asserted to be a GTIN that passes its check digit.
    fn seed_product_with_declaration(state: &AppState) -> i64 {
        create_product_as_admin(
            state,
            SaveProductRequest {
                barcode: Some("4006381333931".to_string()),
                barcode_kind: Some("gtin".to_string()),
                ..product_request_with_declaration("DEK-GAP-COMPLETE")
            },
        )
        .expect("product with declaration data should save")
        .id
    }

    #[test]
    fn gtin_check_digit_is_validated_only_for_real_gtins() {
        assert!(is_valid_gtin("4006381333931"), "valid EAN-13");
        assert!(is_valid_gtin("96385074"), "valid EAN-8");
        assert!(is_valid_gtin("036000291452"), "valid UPC-A (GTIN-12)");
        assert!(is_valid_gtin("10614141000415"), "valid GTIN-14");
        assert!(!is_valid_gtin("4006381333932"), "wrong check digit");
        assert!(!is_valid_gtin("123"), "wrong length");
        assert!(!is_valid_gtin("40063813339A1"), "non-digit");
        assert!(!is_valid_gtin(""), "empty");
    }

    #[test]
    fn declaration_gaps_report_separates_unclassified_barcodes_from_missing_data() {
        with_catalog_state("declaration_gaps", |state| {
            sign_in_admin(state);
            let missing = seed_product_without_declaration(state);
            let complete = seed_product_with_declaration(state);

            let rows = declaration_gaps(state.db()).expect("report should run");

            let row = rows
                .iter()
                .find(|row| row.product_id == missing)
                .expect("gap listed");
            assert!(row.missing_fields.contains(&"manufacturerName".to_string()));
            assert!(row.missing_fields.contains(&"countryOfOrigin".to_string()));
            assert!(
                row.barcode_unclassified,
                "NULL means unclassified, never 'is a GTIN'"
            );
            assert!(
                !row.gtin_check_digit_invalid,
                "an unclassified barcode is never check-digit tested"
            );

            assert!(
                rows.iter().all(|row| row.product_id != complete),
                "a complete product is not a gap"
            );
        });
    }

    #[test]
    fn declaration_gaps_never_check_digit_tests_an_in_house_code() {
        with_catalog_state("declaration_gaps_internal_code", |state| {
            sign_in_admin(state);
            // An in-house printed code that would fail the GTIN check digit.
            let internal = create_product_as_admin(
                state,
                SaveProductRequest {
                    barcode: Some("4006381333932".to_string()),
                    barcode_kind: Some("internal".to_string()),
                    ..product_request_with_declaration("DEK-GAP-INTERNAL")
                },
            )
            .expect("in-house coded product should save")
            .id;

            let declared_gtin = create_product_as_admin(
                state,
                SaveProductRequest {
                    barcode: Some("8600000000041".to_string()),
                    barcode_kind: Some("gtin".to_string()),
                    ..product_request_with_declaration("DEK-GAP-BAD-GTIN")
                },
            )
            .expect("product declared as GTIN should save")
            .id;

            let rows = declaration_gaps(state.db()).expect("report should run");

            assert!(
                rows.iter().all(|row| row.product_id != internal),
                "an in-house code must never be reported as a broken GTIN"
            );

            let row = rows
                .iter()
                .find(|row| row.product_id == declared_gtin)
                .expect("a barcode asserted to be a GTIN is check-digit tested");
            assert!(row.gtin_check_digit_invalid);
            assert!(!row.barcode_unclassified, "the kind was asserted");
            assert!(
                row.missing_fields.is_empty(),
                "the identity data is on file; only the GTIN is wrong"
            );
        });
    }

    #[test]
    fn catalog_declaration_gaps_rejected_for_cashier() {
        let path = test_database_path("catalog_declaration_gaps_rejected_for_cashier");

        {
            let db = Db::new(path.clone()).expect("database should initialize");
            seed_catalog(&db);
            let state = AppState::new(db);
            sign_in_cashier(&state);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = catalog_declaration_gaps(app.state::<AppState>())
                .expect_err("cashier should not read the declaration-gaps report");

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
