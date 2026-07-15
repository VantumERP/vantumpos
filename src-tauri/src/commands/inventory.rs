use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockListQuery {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub stock_state: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockListItem {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub unit_of_measure: String,
    pub current_quantity_milli: i64,
    pub minimum_stock_milli: i64,
    pub low_stock: bool,
    pub sale_price_minor: i64,
    pub last_movement_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockListResult {
    pub items: Vec<StockListItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryMovementType {
    Receive,
    Correction,
    WriteOff,
}

impl InventoryMovementType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Receive => "receive",
            Self::Correction => "correction",
            Self::WriteOff => "write_off",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryAdjustmentRequest {
    pub product_id: i64,
    pub quantity_milli: i64,
    pub reason: Option<String>,
    pub user_id: Option<i64>,
    pub purchase_price_minor: Option<i64>,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryAdjustmentResult {
    pub product_id: i64,
    pub movement_id: i64,
    pub movement_type: String,
    pub quantity_milli: i64,
    pub previous_quantity_milli: i64,
    pub new_quantity_milli: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryLedgerMovement {
    pub id: i64,
    pub movement_type: String,
    pub quantity_milli: i64,
    pub resulting_quantity_milli: i64,
    pub reason: Option<String>,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductLedger {
    pub product_id: i64,
    pub product: StockListItem,
    pub movements: Vec<InventoryLedgerMovement>,
}

#[tauri::command]
pub fn inventory_list_stock(
    state: State<'_, AppState>,
    query: StockListQuery,
) -> Result<StockListResult, CommandError> {
    let connection = state.db().open()?;
    list_stock_for_connection(&connection, query).map_err(Into::into)
}

#[tauri::command]
pub fn inventory_get_product_ledger(
    state: State<'_, AppState>,
    product_id: i64,
) -> Result<ProductLedger, CommandError> {
    let connection = state.db().open()?;
    get_product_ledger_for_connection(&connection, product_id).map_err(Into::into)
}

#[tauri::command]
pub fn inventory_receive(
    state: State<'_, AppState>,
    request: InventoryAdjustmentRequest,
) -> Result<InventoryAdjustmentResult, CommandError> {
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::Receive,
        request,
        &created_at,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn inventory_correct(
    state: State<'_, AppState>,
    request: InventoryAdjustmentRequest,
) -> Result<InventoryAdjustmentResult, CommandError> {
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::Correction,
        request,
        &created_at,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn inventory_write_off(
    state: State<'_, AppState>,
    request: InventoryAdjustmentRequest,
) -> Result<InventoryAdjustmentResult, CommandError> {
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::WriteOff,
        request,
        &created_at,
    )
    .map_err(Into::into)
}

pub fn list_stock_for_connection(
    connection: &Connection,
    query: StockListQuery,
) -> Result<StockListResult, AppError> {
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", value.to_lowercase()));
    let stock_state = normalize_stock_state(query.stock_state.as_deref())?;
    let stock_state_param = stock_state.filter(|value| *value != "all");

    let mut statement = connection.prepare(
        r#"
SELECT
    p.id,
    p.name,
    p.sku,
    p.barcode,
    p.category_id,
    c.name AS category_name,
    p.unit_of_measure,
    COALESCE(ib.quantity_milli, 0) AS current_quantity_milli,
    p.minimum_stock_milli,
    CASE
        WHEN p.minimum_stock_milli > 0
         AND COALESCE(ib.quantity_milli, 0) <= p.minimum_stock_milli
        THEN 1
        ELSE 0
    END AS low_stock,
    p.sale_price_minor,
    (
        SELECT MAX(im.created_at)
        FROM inventory_movements im
        WHERE im.product_id = p.id
    ) AS last_movement_at
FROM products p
LEFT JOIN categories c ON c.id = p.category_id
LEFT JOIN inventory_balances ib ON ib.product_id = p.id
WHERE p.active = 1
  AND (
      ?1 IS NULL
      OR lower(p.name) LIKE ?1
      OR lower(p.sku) LIKE ?1
      OR lower(COALESCE(p.barcode, '')) LIKE ?1
  )
  AND (?2 IS NULL OR p.category_id = ?2)
  AND (
      ?3 IS NULL
      OR (?3 = 'low' AND p.minimum_stock_milli > 0 AND COALESCE(ib.quantity_milli, 0) <= p.minimum_stock_milli)
      OR (?3 = 'zero' AND COALESCE(ib.quantity_milli, 0) = 0)
      OR (?3 = 'negative' AND COALESCE(ib.quantity_milli, 0) < 0)
      OR (?3 = 'out' AND COALESCE(ib.quantity_milli, 0) = 0)
  )
ORDER BY p.name, p.sku
"#,
    )?;

    let items = statement
        .query_map(
            params![search, query.category_id, stock_state_param],
            stock_item_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(StockListResult { items })
}

pub fn get_product_ledger_for_connection(
    connection: &Connection,
    product_id: i64,
) -> Result<ProductLedger, AppError> {
    let product = get_stock_item_for_product(connection, product_id)?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;
    let mut statement = connection.prepare(
        r#"
SELECT
    id,
    movement_type,
    quantity_milli,
    reason,
    reference_type,
    reference_id,
    created_at
FROM inventory_movements
WHERE product_id = ?1
ORDER BY created_at ASC, id ASC
"#,
    )?;

    let raw_movements = statement
        .query_map(params![product_id], |row| {
            Ok(RawLedgerMovement {
                id: row.get(0)?,
                movement_type: row.get(1)?,
                quantity_milli: row.get(2)?,
                reason: row.get(3)?,
                reference_type: row.get(4)?,
                reference_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let movement_total = raw_movements
        .iter()
        .map(|movement| movement.quantity_milli)
        .sum::<i64>();
    let mut running_quantity = product.current_quantity_milli - movement_total;
    let mut movements = Vec::with_capacity(raw_movements.len());

    for movement in raw_movements {
        running_quantity = running_quantity
            .checked_add(movement.quantity_milli)
            .ok_or_else(quantity_overflow_error)?;
        movements.push(InventoryLedgerMovement {
            id: movement.id,
            movement_type: movement.movement_type,
            quantity_milli: movement.quantity_milli,
            resulting_quantity_milli: running_quantity,
            reason: movement.reason,
            reference_type: movement.reference_type,
            reference_id: movement.reference_id,
            created_at: movement.created_at,
        });
    }

    movements.reverse();

    Ok(ProductLedger {
        product_id,
        product,
        movements,
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StockMovementWrite<'a> {
    pub product_id: i64,
    pub movement_type: &'a str,
    pub quantity_milli: i64,
    pub reason: Option<&'a str>,
    pub reference_type: Option<&'a str>,
    pub reference_id: Option<i64>,
    pub user_id: Option<i64>,
    pub created_at: &'a str,
    /// When true, the caller has already authorized selling below stock
    /// (shop-level setting or a per-sale override), so the negative-stock guard
    /// is bypassed even for a product without `allow_negative_stock`.
    pub allow_overselling: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StockWriteOutcome {
    pub movement_id: i64,
    pub previous_quantity_milli: i64,
    pub new_quantity_milli: i64,
}

/// Single source of truth for `inventory_movements` + `inventory_balances`
/// writes, shared by the inventory-adjustment path and the sales path so the
/// two cannot drift in balance math or negative-stock rules. The caller must
/// supply an already-open transaction; this function does not commit.
pub(crate) fn write_stock_movement(
    tx: &Connection,
    write: StockMovementWrite<'_>,
) -> Result<StockWriteOutcome, AppError> {
    let (previous_quantity_milli, allow_negative_stock): (i64, bool) = tx
        .query_row(
            r#"
SELECT COALESCE(ib.quantity_milli, 0), p.allow_negative_stock = 1
FROM products p
LEFT JOIN inventory_balances ib ON ib.product_id = p.id
WHERE p.id = ?1
"#,
            params![write.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;

    let new_quantity_milli = previous_quantity_milli
        .checked_add(write.quantity_milli)
        .ok_or_else(quantity_overflow_error)?;

    if new_quantity_milli < 0 && !allow_negative_stock && !write.allow_overselling {
        return Err(AppError::business(
            "insufficient_stock",
            "Nema dovoljno zaliha.",
        ));
    }

    tx.execute(
        r#"
INSERT INTO inventory_movements (
    product_id,
    movement_type,
    quantity_milli,
    reason,
    reference_type,
    reference_id,
    user_id,
    created_at
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
"#,
        params![
            write.product_id,
            write.movement_type,
            write.quantity_milli,
            write.reason,
            write.reference_type,
            write.reference_id,
            write.user_id,
            write.created_at,
        ],
    )?;
    let movement_id = tx.last_insert_rowid();

    tx.execute(
        r#"
INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
VALUES (?1, ?2, ?3)
ON CONFLICT(product_id) DO UPDATE SET
    quantity_milli = excluded.quantity_milli,
    updated_at = excluded.updated_at
"#,
        params![write.product_id, new_quantity_milli, write.created_at],
    )?;

    Ok(StockWriteOutcome {
        movement_id,
        previous_quantity_milli,
        new_quantity_milli,
    })
}

pub fn apply_inventory_adjustment(
    connection: &mut Connection,
    movement_type: InventoryMovementType,
    request: InventoryAdjustmentRequest,
    created_at: &str,
) -> Result<InventoryAdjustmentResult, AppError> {
    validate_adjustment_request(movement_type, &request)?;

    let delta_quantity_milli = match movement_type {
        InventoryMovementType::Receive => request.quantity_milli,
        InventoryMovementType::Correction => request.quantity_milli,
        InventoryMovementType::WriteOff => request.quantity_milli.saturating_neg(),
    };
    let reason = normalized_optional_text(request.reason.as_deref());
    let reference_type = normalized_optional_text(request.reference_type.as_deref());
    let tx = connection.transaction()?;

    if let Some(purchase_price_minor) = request.purchase_price_minor {
        tx.execute(
            "UPDATE products
             SET purchase_price_minor = ?1, updated_at = ?2
             WHERE id = ?3",
            params![purchase_price_minor, created_at, request.product_id],
        )?;
    }

    let outcome = write_stock_movement(
        &tx,
        StockMovementWrite {
            product_id: request.product_id,
            movement_type: movement_type.as_str(),
            quantity_milli: delta_quantity_milli,
            reason: reason.as_deref(),
            reference_type: reference_type.as_deref(),
            reference_id: request.reference_id,
            user_id: request.user_id,
            created_at,
            allow_overselling: false,
        },
    )?;

    tx.commit()?;

    Ok(InventoryAdjustmentResult {
        product_id: request.product_id,
        movement_id: outcome.movement_id,
        movement_type: movement_type.as_str().to_string(),
        quantity_milli: delta_quantity_milli,
        previous_quantity_milli: outcome.previous_quantity_milli,
        new_quantity_milli: outcome.new_quantity_milli,
        created_at: created_at.to_string(),
    })
}

#[derive(Clone, Debug)]
struct RawLedgerMovement {
    id: i64,
    movement_type: String,
    quantity_milli: i64,
    reason: Option<String>,
    reference_type: Option<String>,
    reference_id: Option<i64>,
    created_at: String,
}

fn get_stock_item_for_product(
    connection: &Connection,
    product_id: i64,
) -> Result<Option<StockListItem>, AppError> {
    connection
        .query_row(
            r#"
SELECT
    p.id,
    p.name,
    p.sku,
    p.barcode,
    p.category_id,
    c.name AS category_name,
    p.unit_of_measure,
    COALESCE(ib.quantity_milli, 0) AS current_quantity_milli,
    p.minimum_stock_milli,
    CASE
        WHEN p.minimum_stock_milli > 0
         AND COALESCE(ib.quantity_milli, 0) <= p.minimum_stock_milli
        THEN 1
        ELSE 0
    END AS low_stock,
    p.sale_price_minor,
    (
        SELECT MAX(im.created_at)
        FROM inventory_movements im
        WHERE im.product_id = p.id
    ) AS last_movement_at
FROM products p
LEFT JOIN categories c ON c.id = p.category_id
LEFT JOIN inventory_balances ib ON ib.product_id = p.id
WHERE p.id = ?1
"#,
            params![product_id],
            stock_item_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn stock_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StockListItem> {
    Ok(StockListItem {
        product_id: row.get(0)?,
        product_name: row.get(1)?,
        sku: row.get(2)?,
        barcode: row.get(3)?,
        category_id: row.get(4)?,
        category_name: row.get(5)?,
        unit_of_measure: row.get(6)?,
        current_quantity_milli: row.get(7)?,
        minimum_stock_milli: row.get(8)?,
        low_stock: row.get::<_, i64>(9)? != 0,
        sale_price_minor: row.get(10)?,
        last_movement_at: row.get(11)?,
    })
}

fn validate_adjustment_request(
    movement_type: InventoryMovementType,
    request: &InventoryAdjustmentRequest,
) -> Result<(), AppError> {
    if request.product_id <= 0 {
        return Err(validation_error("Artikal nije ispravan.", "productId"));
    }

    if request.quantity_milli == 0 {
        return Err(validation_error("Količina je obavezna.", "quantityMilli"));
    }

    if matches!(
        movement_type,
        InventoryMovementType::Receive | InventoryMovementType::WriteOff
    ) && request.quantity_milli < 0
    {
        return Err(validation_error(
            "Količina mora biti pozitivna.",
            "quantityMilli",
        ));
    }

    if matches!(
        movement_type,
        InventoryMovementType::Correction | InventoryMovementType::WriteOff
    ) && normalized_optional_text(request.reason.as_deref()).is_none()
    {
        return Err(validation_error("Razlog je obavezan.", "reason"));
    }

    if let Some(purchase_price_minor) = request.purchase_price_minor {
        if purchase_price_minor < 0 {
            return Err(validation_error(
                "Nabavna cena nije ispravna.",
                "purchasePriceMinor",
            ));
        }
    }

    Ok(())
}

fn normalize_stock_state(value: Option<&str>) -> Result<Option<&'static str>, AppError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(Some("all")),
        Some("all") => Ok(Some("all")),
        Some("low") => Ok(Some("low")),
        Some("zero") => Ok(Some("zero")),
        Some("negative") => Ok(Some("negative")),
        Some("out") => Ok(Some("out")),
        Some(_) => Err(validation_error(
            "Filter stanja nije ispravan.",
            "stockState",
        )),
    }
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn validation_error(message: &str, field: &str) -> AppError {
    AppError::validation(message, serde_json::json!({ "field": field }))
}

fn quantity_overflow_error() -> AppError {
    validation_error("Količina nije ispravna.", "quantityMilli")
}

fn now_utc_string() -> Result<String, AppError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::InvalidState(format!("Vreme sistema nije ispravno: {error}")))
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::app_error::CommandError;
    use crate::commands::inventory::{
        apply_inventory_adjustment, get_product_ledger_for_connection, list_stock_for_connection,
        write_stock_movement, InventoryAdjustmentRequest, InventoryMovementType, StockListQuery,
        StockMovementWrite,
    };
    use crate::db::{test_database_path, Db};

    fn with_connection(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let mut connection = db.open().expect("database should open");
            seed_required_data(&mut connection);
            test(&mut connection);
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }

    fn seed_required_data(connection: &mut Connection) {
        connection
            .execute(
                "INSERT OR IGNORE INTO users (id, username, display_name, role, created_at, updated_at)
                 VALUES (1, 'admin', 'Administrator', 'admin', '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("user should insert");

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
                    allow_negative_stock,
                    created_at,
                    updated_at
                 )
                 VALUES (
                    1,
                    'Mleko 1 l',
                    'MLEKO-1L',
                    '8600000000011',
                    1,
                    'kom',
                    12999,
                    9500,
                    1,
                    5000,
                    0,
                    '2026-06-18T10:00:00Z',
                    '2026-06-18T10:00:00Z'
                 )",
                [],
            )
            .expect("product should insert");
    }

    fn adjustment(quantity_milli: i64, reason: &str) -> InventoryAdjustmentRequest {
        InventoryAdjustmentRequest {
            product_id: 1,
            quantity_milli,
            reason: Some(reason.to_string()),
            user_id: Some(1),
            purchase_price_minor: None,
            reference_type: None,
            reference_id: None,
        }
    }

    fn current_balance(connection: &Connection) -> i64 {
        connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("balance should query")
    }

    #[test]
    fn receive_stock_creates_movement_and_increases_balance() {
        with_connection(
            "receive_stock_creates_movement_and_increases_balance",
            |connection| {
                let result = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(3000, "Prijem robe"),
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed");

                assert_eq!(result.previous_quantity_milli, 0);
                assert_eq!(result.new_quantity_milli, 3000);
                assert_eq!(current_balance(connection), 3000);

                let movement: (String, i64, Option<String>, Option<i64>) = connection
                    .query_row(
                        "SELECT movement_type, quantity_milli, reason, user_id
                         FROM inventory_movements
                         WHERE id = ?1",
                        params![result.movement_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .expect("movement should query");

                assert_eq!(
                    movement,
                    (
                        "receive".to_string(),
                        3000,
                        Some("Prijem robe".to_string()),
                        Some(1)
                    )
                );
            },
        );
    }

    #[test]
    fn write_stock_movement_sets_absolute_balance() {
        with_connection("write_stock_movement_sets_absolute_balance", |connection| {
            connection
                .execute(
                    "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                     VALUES (1, 5000, '2026-06-18T10:00:00Z')",
                    [],
                )
                .expect("balance should seed");

            let outcome = write_stock_movement(
                connection,
                StockMovementWrite {
                    product_id: 1,
                    movement_type: "sale",
                    quantity_milli: -2000,
                    reason: Some("Prodaja"),
                    reference_type: Some("sale"),
                    reference_id: Some(7),
                    user_id: Some(1),
                    created_at: "2026-06-18T12:00:00Z",
                    allow_overselling: false,
                },
            )
            .expect("write should succeed");

            assert_eq!(outcome.previous_quantity_milli, 5000);
            assert_eq!(outcome.new_quantity_milli, 3000);
            assert_eq!(current_balance(connection), 3000);

            let movement: (String, i64, Option<String>, Option<i64>) = connection
                .query_row(
                    "SELECT movement_type, quantity_milli, reason, user_id
                     FROM inventory_movements WHERE id = ?1",
                    params![outcome.movement_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("movement should query");
            assert_eq!(
                movement,
                (
                    "sale".to_string(),
                    -2000,
                    Some("Prodaja".to_string()),
                    Some(1)
                )
            );
        });
    }

    #[test]
    fn write_stock_movement_blocks_negative_for_non_negative_product() {
        with_connection(
            "write_stock_movement_blocks_negative_for_non_negative_product",
            |connection| {
                let error = write_stock_movement(
                    connection,
                    StockMovementWrite {
                        product_id: 1,
                        movement_type: "sale",
                        quantity_milli: -1000,
                        reason: Some("Prodaja"),
                        reference_type: Some("sale"),
                        reference_id: Some(42),
                        user_id: Some(1),
                        created_at: "2026-06-18T12:00:00Z",
                        allow_overselling: false,
                    },
                )
                .expect_err("oversell should fail");
                let command_error = CommandError::from(error);
                assert_eq!(command_error.code, "insufficient_stock");

                let movement_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM inventory_movements WHERE product_id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("movement count should query");
                assert_eq!(movement_count, 0);
            },
        );
    }

    #[test]
    fn write_off_rejects_negative_result_and_rolls_back_balance() {
        with_connection(
            "write_off_rejects_negative_result_and_rolls_back_balance",
            |connection| {
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(1000, "Pocetno stanje"),
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed");

                let error = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::WriteOff,
                    adjustment(2000, "Lom"),
                    "2026-06-18T13:00:00Z",
                )
                .expect_err("write-off should fail");
                let command_error = CommandError::from(error);

                assert_eq!(command_error.code, "insufficient_stock");
                assert_eq!(current_balance(connection), 1000);

                let movement_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM inventory_movements WHERE product_id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("movement count should query");
                assert_eq!(movement_count, 1);
            },
        );
    }

    #[test]
    fn ledger_returns_newest_movements_with_resulting_balance() {
        with_connection(
            "ledger_returns_newest_movements_with_resulting_balance",
            |connection| {
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(3000, "Prijem robe"),
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed");
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Correction,
                    adjustment(-500, "Korekcija popisa"),
                    "2026-06-18T13:00:00Z",
                )
                .expect("correction should succeed");

                let ledger =
                    get_product_ledger_for_connection(connection, 1).expect("ledger should load");

                assert_eq!(
                    ledger
                        .movements
                        .iter()
                        .map(|movement| (
                            movement.movement_type.clone(),
                            movement.quantity_milli,
                            movement.resulting_quantity_milli
                        ))
                        .collect::<Vec<_>>(),
                    vec![
                        ("correction".to_string(), -500, 2500),
                        ("receive".to_string(), 3000, 3000)
                    ]
                );
            },
        );
    }

    #[test]
    fn list_stock_applies_low_stock_filter() {
        with_connection("list_stock_applies_low_stock_filter", |connection| {
            apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(3000, "Prijem robe"),
                "2026-06-18T12:00:00Z",
            )
            .expect("receive should succeed");

            let stock = list_stock_for_connection(
                connection,
                StockListQuery {
                    search: Some("mleko".to_string()),
                    category_id: Some(1),
                    stock_state: Some("low".to_string()),
                },
            )
            .expect("stock should load");

            assert_eq!(stock.items.len(), 1);
            assert_eq!(stock.items[0].product_name, "Mleko 1 l");
            assert!(stock.items[0].low_stock);
        });
    }
}
