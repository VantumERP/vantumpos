use std::collections::BTreeMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::db::Db;
use crate::state::AppState;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptSearchQuery {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub receipt_number: Option<String>,
    pub cashier: Option<String>,
    pub shift_id: Option<i64>,
    pub payment_method: Option<String>,
    pub product: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptSummary {
    pub id: i64,
    pub receipt_number: String,
    pub created_at: String,
    pub cashier_name: String,
    pub shift_id: i64,
    pub status: String,
    pub fiscal_status: String,
    pub document_type: String,
    pub payment_methods: Vec<String>,
    pub total_minor: i64,
    pub linked_document_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptSearchResult {
    pub receipts: Vec<ReceiptSummary>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptItem {
    pub id: i64,
    pub product_id: Option<i64>,
    pub product_name: String,
    pub product_sku: String,
    pub product_barcode: Option<String>,
    pub quantity_milli: i64,
    pub unit_price_minor: i64,
    pub discount_minor: i64,
    pub tax_rate_basis_points: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub returned_quantity_milli: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptPayment {
    pub id: i64,
    pub payment_method: String,
    pub amount_minor: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLink {
    pub id: i64,
    pub receipt_number: String,
    pub document_type: String,
    pub status: String,
    pub created_at: String,
    pub total_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDetail {
    pub id: i64,
    pub receipt_number: String,
    pub created_at: String,
    pub cashier_name: String,
    pub shift_id: i64,
    pub status: String,
    pub fiscal_status: String,
    pub document_type: String,
    pub payment_methods: Vec<String>,
    pub total_minor: i64,
    pub linked_document_count: i64,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub original_sale_id: Option<i64>,
    pub original_receipt_number: Option<String>,
    pub void_reason: Option<String>,
    pub return_reason: Option<String>,
    pub items: Vec<ReceiptItem>,
    pub payments: Vec<ReceiptPayment>,
    pub linked_documents: Vec<ReceiptLink>,
    pub can_void: bool,
    pub can_return: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidReceiptRequest {
    pub receipt_id: i64,
    pub user_id: i64,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnItemRequest {
    pub sale_item_id: i64,
    pub quantity_milli: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnItemsRequest {
    pub receipt_id: i64,
    pub user_id: i64,
    pub reason: String,
    pub items: Vec<ReturnItemRequest>,
    #[serde(default)]
    pub refund_tender: Option<String>,
}

#[derive(Clone, Debug)]
struct ReceiptHeader {
    id: i64,
    receipt_number: String,
    created_at: String,
    cashier_name: String,
    shift_id: i64,
    status: String,
    fiscal_status: String,
    document_type: String,
    subtotal_minor: i64,
    discount_minor: i64,
    tax_minor: i64,
    total_minor: i64,
    original_sale_id: Option<i64>,
    original_receipt_number: Option<String>,
    void_reason: Option<String>,
    return_reason: Option<String>,
    payment_methods: Vec<String>,
}

#[derive(Clone, Debug)]
struct OriginalSaleItem {
    id: i64,
    product_id: Option<i64>,
    product_name: String,
    product_sku: String,
    product_barcode: Option<String>,
    quantity_milli: i64,
    unit_price_minor: i64,
    discount_minor: i64,
    tax_rate_basis_points: i64,
    tax_minor: i64,
    total_minor: i64,
}

#[tauri::command]
pub fn receipts_search(
    state: State<'_, AppState>,
    query: ReceiptSearchQuery,
) -> Result<ReceiptSearchResult, CommandError> {
    search_receipts(state.db(), query).map_err(Into::into)
}

#[tauri::command]
pub fn receipts_get(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<ReceiptDetail>, CommandError> {
    get_receipt_detail(state.db(), id)
        .map(Some)
        .or_else(|error| {
            if matches!(error, AppError::NotFound(_)) {
                Ok(None)
            } else {
                Err(error.into())
            }
        })
}

#[tauri::command]
pub fn receipts_void(
    state: State<'_, AppState>,
    request: VoidReceiptRequest,
) -> Result<ReceiptDetail, CommandError> {
    void_receipt(state.db(), request)
}

#[tauri::command]
pub fn receipts_return_items(
    state: State<'_, AppState>,
    request: ReturnItemsRequest,
) -> Result<ReceiptDetail, CommandError> {
    return_items(state.db(), request)
}

pub fn search_receipts(
    db: &Db,
    query: ReceiptSearchQuery,
) -> Result<ReceiptSearchResult, AppError> {
    let connection = db.open()?;
    let mut conditions = vec!["1 = 1".to_string()];
    let mut values = Vec::<Value>::new();

    if let Some(date_from) = normalized_optional_text(query.date_from.or(query.from).as_deref()) {
        conditions.push("s.created_at >= ?".to_string());
        values.push(Value::Text(date_from));
    }

    if let Some(date_to) = normalized_optional_text(query.date_to.or(query.to).as_deref()) {
        conditions.push("s.created_at <= ?".to_string());
        values.push(Value::Text(date_to));
    }

    if let Some(receipt_number) = like_value(query.receipt_number.as_deref()) {
        conditions.push("lower(s.local_receipt_number) LIKE ?".to_string());
        values.push(Value::Text(receipt_number));
    }

    if let Some(cashier) = like_value(query.cashier.as_deref()) {
        conditions.push("lower(u.display_name) LIKE ?".to_string());
        values.push(Value::Text(cashier));
    }

    if let Some(shift_id) = query.shift_id {
        conditions.push("s.shift_id = ?".to_string());
        values.push(Value::Integer(shift_id));
    }

    if let Some(payment_method) = normalized_optional_text(query.payment_method.as_deref()) {
        validate_payment_method(&payment_method)?;
        conditions.push(
            "EXISTS (
                SELECT 1 FROM sale_payments sp
                WHERE sp.sale_id = s.id AND sp.payment_method = ?
            )"
            .to_string(),
        );
        values.push(Value::Text(payment_method));
    }

    if let Some(product) = like_value(query.product.as_deref()) {
        conditions.push(
            "EXISTS (
                SELECT 1 FROM sale_items si
                WHERE si.sale_id = s.id
                  AND (
                    lower(si.product_name) LIKE ?
                    OR lower(si.product_sku) LIKE ?
                    OR lower(COALESCE(si.product_barcode, '')) LIKE ?
                  )
            )"
            .to_string(),
        );
        values.push(Value::Text(product.clone()));
        values.push(Value::Text(product.clone()));
        values.push(Value::Text(product));
    }

    let sql = format!(
        r#"
SELECT
    s.id,
    s.local_receipt_number,
    s.created_at,
    u.display_name,
    s.shift_id,
    s.status,
    s.fiscal_status,
    s.document_type,
    s.total_minor,
    COALESCE((
        SELECT GROUP_CONCAT(DISTINCT sp.payment_method)
        FROM sale_payments sp
        WHERE sp.sale_id = s.id
    ), '') AS payment_methods,
    (
        SELECT COUNT(*)
        FROM sales linked
        WHERE linked.original_sale_id = s.id
    ) AS linked_document_count
FROM sales s
JOIN users u ON u.id = s.cashier_id
WHERE {}
ORDER BY s.created_at DESC, s.id DESC
LIMIT 100
"#,
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let receipts = statement
        .query_map(params_from_iter(values.iter()), receipt_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total = receipts.len();

    Ok(ReceiptSearchResult { receipts, total })
}

pub fn get_receipt_detail(db: &Db, receipt_id: i64) -> Result<ReceiptDetail, AppError> {
    let connection = db.open()?;
    load_receipt_detail(&connection, receipt_id)
}

fn current_open_shift_id(connection: &Connection) -> Result<i64, AppError> {
    connection
        .query_row(
            "SELECT id FROM shifts
             WHERE status = 'open'
             ORDER BY opened_at DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::business("shift_required", "Smena nije otvorena."))
}

pub fn void_receipt(db: &Db, request: VoidReceiptRequest) -> Result<ReceiptDetail, CommandError> {
    validate_receipt_id(request.receipt_id)?;
    validate_user_id(request.user_id)?;
    let reason = require_reason(&request.reason, "reason")?;
    let now = utc_now()?;
    let mut connection = db.open()?;

    {
        let tx = connection.transaction().map_err(AppError::from)?;
        let header = load_receipt_header(&tx, request.receipt_id)?
            .ok_or_else(|| AppError::not_found("Račun nije pronađen."))?;

        ensure_voidable(&tx, &header)?;
        let shift_id = current_open_shift_id(&tx)?;

        let items = load_original_sale_items(&tx, request.receipt_id)?;
        if items.is_empty() {
            return Err(AppError::business("invalid_receipt_state", "Račun nema stavke.").into());
        }

        let linked_number = next_linked_receipt_number(&tx, request.receipt_id, "STO", &header)?;
        tx.execute(
            "INSERT INTO sales (
                local_receipt_number,
                shift_id,
                cashier_id,
                status,
                fiscal_status,
                document_type,
                original_sale_id,
                subtotal_minor,
                discount_minor,
                tax_minor,
                total_minor,
                void_reason,
                created_at,
                updated_at
             )
             VALUES (?1, ?2, ?3, 'voided', 'not_fiscalized', 'void', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                linked_number,
                shift_id,
                request.user_id,
                request.receipt_id,
                header.subtotal_minor,
                header.discount_minor,
                header.tax_minor,
                header.total_minor,
                reason,
                now,
            ],
        )
        .map_err(AppError::from)?;
        let linked_sale_id = tx.last_insert_rowid();

        for item in items {
            let returned_quantity = item.quantity_milli.abs();
            tx.execute(
                "INSERT INTO sale_items (
                    sale_id,
                    product_id,
                    product_name,
                    product_sku,
                    product_barcode,
                    quantity_milli,
                    unit_price_minor,
                    discount_minor,
                    tax_rate_basis_points,
                    tax_minor,
                    total_minor,
                    original_sale_item_id
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    linked_sale_id,
                    item.product_id,
                    item.product_name,
                    item.product_sku,
                    item.product_barcode,
                    -returned_quantity,
                    item.unit_price_minor,
                    item.discount_minor,
                    item.tax_rate_basis_points,
                    item.tax_minor,
                    item.total_minor,
                    item.id,
                ],
            )
            .map_err(AppError::from)?;

            restore_inventory(
                &tx,
                item.product_id,
                returned_quantity,
                "void",
                &reason,
                "sale_void",
                linked_sale_id,
                request.user_id,
                &now,
            )?;
        }

        let original_payments: Vec<(String, i64)> = {
            let mut statement = tx
                .prepare(
                    "SELECT payment_method, amount_minor FROM sale_payments WHERE sale_id = ?1",
                )
                .map_err(AppError::from)?;
            let mapped = statement
                .query_map(params![request.receipt_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(AppError::from)?;
            mapped
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?
        };
        for (method, amount) in original_payments {
            if amount == 0 {
                continue;
            }
            tx.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![linked_sale_id, method, -amount, now],
            )
            .map_err(AppError::from)?;
        }

        tx.execute(
            "UPDATE sales
             SET status = 'voided', void_reason = ?1, updated_at = ?2
             WHERE id = ?3",
            params![reason, now, request.receipt_id],
        )
        .map_err(AppError::from)?;

        tx.commit().map_err(AppError::from)?;
    }

    get_receipt_detail(db, request.receipt_id).map_err(Into::into)
}

pub fn return_items(db: &Db, request: ReturnItemsRequest) -> Result<ReceiptDetail, CommandError> {
    validate_receipt_id(request.receipt_id)?;
    validate_user_id(request.user_id)?;
    let reason = require_reason(&request.reason, "reason")?;
    let requested = normalize_return_items(&request.items)?;
    let refund_tender = request
        .refund_tender
        .as_deref()
        .unwrap_or("cash")
        .to_string();
    validate_payment_method(&refund_tender)?;
    let now = utc_now()?;
    let mut connection = db.open()?;

    {
        let tx = connection.transaction().map_err(AppError::from)?;
        let header = load_receipt_header(&tx, request.receipt_id)?
            .ok_or_else(|| AppError::not_found("Račun nije pronađen."))?;

        ensure_returnable(&tx, &header)?;
        let shift_id = current_open_shift_id(&tx)?;

        let mut return_items = Vec::<(OriginalSaleItem, i64)>::new();
        for (sale_item_id, quantity_milli) in requested {
            let item = load_original_sale_item(&tx, request.receipt_id, sale_item_id)?
                .ok_or_else(|| AppError::not_found("Stavka računa nije pronađena."))?;
            let already_returned = returned_quantity_for_item(&tx, item.id)?;
            let original_quantity = item.quantity_milli.abs();
            let remaining = original_quantity.saturating_sub(already_returned);

            if quantity_milli > remaining {
                return Err(AppError::business_with_details(
                    "return_quantity_exceeded",
                    "Količina za povrat je veća od raspoložive količine.",
                    serde_json::json!({
                        "saleItemId": item.id,
                        "remainingQuantityMilli": remaining
                    }),
                )
                .into());
            }

            return_items.push((item, quantity_milli));
        }

        let subtotal_minor = return_items
            .iter()
            .map(|(item, quantity)| proportional_unit_total(item.unit_price_minor, *quantity))
            .sum::<i64>();
        let discount_minor = return_items
            .iter()
            .map(|(item, quantity)| {
                proportional_amount(item.discount_minor, *quantity, item.quantity_milli.abs())
            })
            .sum::<i64>();
        let tax_minor = return_items
            .iter()
            .map(|(item, quantity)| {
                proportional_amount(item.tax_minor, *quantity, item.quantity_milli.abs())
            })
            .sum::<i64>();
        let total_minor = return_items
            .iter()
            .map(|(item, quantity)| {
                proportional_amount(item.total_minor, *quantity, item.quantity_milli.abs())
            })
            .sum::<i64>();

        let linked_number = next_linked_receipt_number(&tx, request.receipt_id, "POV", &header)?;
        tx.execute(
            "INSERT INTO sales (
                local_receipt_number,
                shift_id,
                cashier_id,
                status,
                fiscal_status,
                document_type,
                original_sale_id,
                subtotal_minor,
                discount_minor,
                tax_minor,
                total_minor,
                return_reason,
                created_at,
                updated_at
             )
             VALUES (?1, ?2, ?3, 'refunded', 'not_fiscalized', 'return', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                linked_number,
                shift_id,
                request.user_id,
                request.receipt_id,
                subtotal_minor,
                discount_minor,
                tax_minor,
                total_minor,
                reason,
                now,
            ],
        )
        .map_err(AppError::from)?;
        let linked_sale_id = tx.last_insert_rowid();

        for (item, quantity_milli) in return_items {
            let item_discount = proportional_amount(
                item.discount_minor,
                quantity_milli,
                item.quantity_milli.abs(),
            );
            let item_tax =
                proportional_amount(item.tax_minor, quantity_milli, item.quantity_milli.abs());
            let item_total =
                proportional_amount(item.total_minor, quantity_milli, item.quantity_milli.abs());

            tx.execute(
                "INSERT INTO sale_items (
                    sale_id,
                    product_id,
                    product_name,
                    product_sku,
                    product_barcode,
                    quantity_milli,
                    unit_price_minor,
                    discount_minor,
                    tax_rate_basis_points,
                    tax_minor,
                    total_minor,
                    original_sale_item_id
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    linked_sale_id,
                    item.product_id,
                    item.product_name,
                    item.product_sku,
                    item.product_barcode,
                    -quantity_milli,
                    item.unit_price_minor,
                    item_discount,
                    item.tax_rate_basis_points,
                    item_tax,
                    item_total,
                    item.id,
                ],
            )
            .map_err(AppError::from)?;

            restore_inventory(
                &tx,
                item.product_id,
                quantity_milli,
                "return",
                &reason,
                "sale_return",
                linked_sale_id,
                request.user_id,
                &now,
            )?;
        }

        if total_minor != 0 {
            tx.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![linked_sale_id, refund_tender, -total_minor, now],
            )
            .map_err(AppError::from)?;
        }

        tx.execute(
            "UPDATE sales
             SET status = 'refunded', return_reason = ?1, updated_at = ?2
             WHERE id = ?3",
            params![reason, now, request.receipt_id],
        )
        .map_err(AppError::from)?;

        tx.commit().map_err(AppError::from)?;
    }

    get_receipt_detail(db, request.receipt_id).map_err(Into::into)
}

fn load_receipt_detail(
    connection: &Connection,
    receipt_id: i64,
) -> Result<ReceiptDetail, AppError> {
    validate_receipt_id(receipt_id)?;
    let header = load_receipt_header(connection, receipt_id)?
        .ok_or_else(|| AppError::not_found("Račun nije pronađen."))?;
    let items = load_receipt_items(connection, receipt_id)?;
    let payments = load_receipt_payments(connection, receipt_id)?;
    let linked_documents = load_linked_documents(connection, receipt_id)?;
    let has_void_document = linked_documents
        .iter()
        .any(|document| document.document_type == "void");
    let has_returnable_items = items
        .iter()
        .any(|item| item.quantity_milli.abs() > item.returned_quantity_milli);
    let can_void =
        header.document_type == "sale" && header.status == "completed" && !has_void_document;
    let can_return =
        header.document_type == "sale" && header.status != "voided" && has_returnable_items;

    Ok(ReceiptDetail {
        id: header.id,
        receipt_number: header.receipt_number,
        created_at: header.created_at,
        cashier_name: header.cashier_name,
        shift_id: header.shift_id,
        status: header.status,
        fiscal_status: header.fiscal_status,
        document_type: header.document_type,
        payment_methods: header.payment_methods,
        total_minor: header.total_minor,
        linked_document_count: linked_documents.len() as i64,
        subtotal_minor: header.subtotal_minor,
        discount_minor: header.discount_minor,
        tax_minor: header.tax_minor,
        original_sale_id: header.original_sale_id,
        original_receipt_number: header.original_receipt_number,
        void_reason: header.void_reason,
        return_reason: header.return_reason,
        items,
        payments,
        linked_documents,
        can_void,
        can_return,
    })
}

fn load_receipt_header(
    connection: &Connection,
    receipt_id: i64,
) -> Result<Option<ReceiptHeader>, AppError> {
    connection
        .query_row(
            r#"
SELECT
    s.id,
    s.local_receipt_number,
    s.created_at,
    u.display_name,
    s.shift_id,
    s.status,
    s.fiscal_status,
    s.document_type,
    s.subtotal_minor,
    s.discount_minor,
    s.tax_minor,
    s.total_minor,
    s.original_sale_id,
    original.local_receipt_number,
    s.void_reason,
    s.return_reason,
    COALESCE((
        SELECT GROUP_CONCAT(DISTINCT sp.payment_method)
        FROM sale_payments sp
        WHERE sp.sale_id = s.id
    ), '') AS payment_methods
FROM sales s
JOIN users u ON u.id = s.cashier_id
LEFT JOIN sales original ON original.id = s.original_sale_id
WHERE s.id = ?1
"#,
            params![receipt_id],
            |row| {
                Ok(ReceiptHeader {
                    id: row.get(0)?,
                    receipt_number: row.get(1)?,
                    created_at: row.get(2)?,
                    cashier_name: row.get(3)?,
                    shift_id: row.get(4)?,
                    status: row.get(5)?,
                    fiscal_status: row.get(6)?,
                    document_type: row.get(7)?,
                    subtotal_minor: row.get(8)?,
                    discount_minor: row.get(9)?,
                    tax_minor: row.get(10)?,
                    total_minor: row.get(11)?,
                    original_sale_id: row.get(12)?,
                    original_receipt_number: row.get(13)?,
                    void_reason: row.get(14)?,
                    return_reason: row.get(15)?,
                    payment_methods: parse_payment_methods(row.get::<_, String>(16)?),
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn load_receipt_items(
    connection: &Connection,
    receipt_id: i64,
) -> Result<Vec<ReceiptItem>, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    si.id,
    si.product_id,
    si.product_name,
    si.product_sku,
    si.product_barcode,
    si.quantity_milli,
    si.unit_price_minor,
    si.discount_minor,
    si.tax_rate_basis_points,
    si.tax_minor,
    si.total_minor,
    COALESCE((
        SELECT SUM(ABS(return_item.quantity_milli))
        FROM sales linked
        JOIN sale_items return_item ON return_item.sale_id = linked.id
        WHERE linked.original_sale_id = si.sale_id
          AND linked.document_type IN ('return', 'void')
          AND return_item.original_sale_item_id = si.id
    ), 0) AS returned_quantity_milli
FROM sale_items si
WHERE si.sale_id = ?1
ORDER BY si.id
"#,
    )?;

    let items = statement
        .query_map(params![receipt_id], |row| {
            Ok(ReceiptItem {
                id: row.get(0)?,
                product_id: row.get(1)?,
                product_name: row.get(2)?,
                product_sku: row.get(3)?,
                product_barcode: row.get(4)?,
                quantity_milli: row.get(5)?,
                unit_price_minor: row.get(6)?,
                discount_minor: row.get(7)?,
                tax_rate_basis_points: row.get(8)?,
                tax_minor: row.get(9)?,
                total_minor: row.get(10)?,
                returned_quantity_milli: row.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(items)
}

fn load_receipt_payments(
    connection: &Connection,
    receipt_id: i64,
) -> Result<Vec<ReceiptPayment>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, payment_method, amount_minor, created_at
         FROM sale_payments
         WHERE sale_id = ?1
         ORDER BY id",
    )?;

    let payments = statement
        .query_map(params![receipt_id], |row| {
            Ok(ReceiptPayment {
                id: row.get(0)?,
                payment_method: row.get(1)?,
                amount_minor: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(payments)
}

fn load_linked_documents(
    connection: &Connection,
    receipt_id: i64,
) -> Result<Vec<ReceiptLink>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, local_receipt_number, document_type, status, created_at, total_minor
         FROM sales
         WHERE original_sale_id = ?1
         ORDER BY created_at, id",
    )?;

    let links = statement
        .query_map(params![receipt_id], |row| {
            Ok(ReceiptLink {
                id: row.get(0)?,
                receipt_number: row.get(1)?,
                document_type: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                total_minor: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(links)
}

fn load_original_sale_items(
    connection: &Connection,
    sale_id: i64,
) -> Result<Vec<OriginalSaleItem>, AppError> {
    let mut statement = connection.prepare(
        "SELECT
            id,
            product_id,
            product_name,
            product_sku,
            product_barcode,
            quantity_milli,
            unit_price_minor,
            discount_minor,
            tax_rate_basis_points,
            tax_minor,
            total_minor
         FROM sale_items
         WHERE sale_id = ?1
         ORDER BY id",
    )?;

    let items = statement
        .query_map(params![sale_id], original_sale_item_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(items)
}

fn load_original_sale_item(
    connection: &Connection,
    sale_id: i64,
    sale_item_id: i64,
) -> Result<Option<OriginalSaleItem>, AppError> {
    connection
        .query_row(
            "SELECT
                id,
                product_id,
                product_name,
                product_sku,
                product_barcode,
                quantity_milli,
                unit_price_minor,
                discount_minor,
                tax_rate_basis_points,
                tax_minor,
                total_minor
             FROM sale_items
             WHERE sale_id = ?1 AND id = ?2",
            params![sale_id, sale_item_id],
            original_sale_item_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn original_sale_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OriginalSaleItem> {
    Ok(OriginalSaleItem {
        id: row.get(0)?,
        product_id: row.get(1)?,
        product_name: row.get(2)?,
        product_sku: row.get(3)?,
        product_barcode: row.get(4)?,
        quantity_milli: row.get(5)?,
        unit_price_minor: row.get(6)?,
        discount_minor: row.get(7)?,
        tax_rate_basis_points: row.get(8)?,
        tax_minor: row.get(9)?,
        total_minor: row.get(10)?,
    })
}

fn receipt_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReceiptSummary> {
    Ok(ReceiptSummary {
        id: row.get(0)?,
        receipt_number: row.get(1)?,
        created_at: row.get(2)?,
        cashier_name: row.get(3)?,
        shift_id: row.get(4)?,
        status: row.get(5)?,
        fiscal_status: row.get(6)?,
        document_type: row.get(7)?,
        total_minor: row.get(8)?,
        payment_methods: parse_payment_methods(row.get::<_, String>(9)?),
        linked_document_count: row.get(10)?,
    })
}

fn ensure_voidable(connection: &Connection, header: &ReceiptHeader) -> Result<(), AppError> {
    if header.document_type != "sale" {
        return Err(AppError::business(
            "invalid_receipt_state",
            "Samo originalni račun može da se stornira.",
        ));
    }

    if header.status == "voided" || has_linked_document(connection, header.id, "void")? {
        return Err(AppError::business(
            "duplicate_void",
            "Račun je već storniran.",
        ));
    }

    if header.status != "completed" {
        return Err(AppError::business(
            "invalid_receipt_state",
            "Storniranje je moguće samo za završen račun bez povrata.",
        ));
    }

    Ok(())
}

fn ensure_returnable(connection: &Connection, header: &ReceiptHeader) -> Result<(), AppError> {
    if header.document_type != "sale" {
        return Err(AppError::business(
            "invalid_receipt_state",
            "Povrat je moguć samo za originalni račun.",
        ));
    }

    if header.status == "voided" || has_linked_document(connection, header.id, "void")? {
        return Err(AppError::business(
            "invalid_receipt_state",
            "Povrat nije moguć za storniran račun.",
        ));
    }

    Ok(())
}

fn has_linked_document(
    connection: &Connection,
    receipt_id: i64,
    document_type: &str,
) -> Result<bool, AppError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sales WHERE original_sale_id = ?1 AND document_type = ?2",
        params![receipt_id, document_type],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn next_linked_receipt_number(
    connection: &Connection,
    receipt_id: i64,
    prefix: &str,
    header: &ReceiptHeader,
) -> Result<String, AppError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sales WHERE original_sale_id = ?1",
        params![receipt_id],
        |row| row.get(0),
    )?;

    Ok(format!("{prefix}-{}-{}", header.receipt_number, count + 1))
}

#[allow(clippy::too_many_arguments)]
fn restore_inventory(
    connection: &Connection,
    product_id: Option<i64>,
    quantity_milli: i64,
    movement_type: &str,
    reason: &str,
    reference_type: &str,
    reference_id: i64,
    user_id: i64,
    created_at: &str,
) -> Result<(), AppError> {
    let Some(product_id) = product_id else {
        return Ok(());
    };

    if quantity_milli <= 0 {
        return Err(AppError::validation(
            "Količina za lager nije ispravna.",
            serde_json::json!({ "field": "quantityMilli" }),
        ));
    }

    let current_quantity = connection
        .query_row(
            "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
            params![product_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let new_quantity = current_quantity
        .checked_add(quantity_milli)
        .ok_or_else(|| {
            AppError::validation(
                "Količina nije ispravna.",
                serde_json::json!({ "field": "quantityMilli" }),
            )
        })?;

    connection.execute(
        "INSERT INTO inventory_movements (
            product_id,
            movement_type,
            quantity_milli,
            reason,
            reference_type,
            reference_id,
            user_id,
            created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            product_id,
            movement_type,
            quantity_milli,
            reason,
            reference_type,
            reference_id,
            user_id,
            created_at,
        ],
    )?;

    connection.execute(
        "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(product_id) DO UPDATE SET
            quantity_milli = excluded.quantity_milli,
            updated_at = excluded.updated_at",
        params![product_id, new_quantity, created_at],
    )?;

    Ok(())
}

fn returned_quantity_for_item(connection: &Connection, sale_item_id: i64) -> Result<i64, AppError> {
    connection
        .query_row(
            "SELECT COALESCE(SUM(ABS(return_item.quantity_milli)), 0)
             FROM sales linked
             JOIN sale_items return_item ON return_item.sale_id = linked.id
             WHERE linked.document_type IN ('return', 'void')
               AND return_item.original_sale_item_id = ?1",
            params![sale_item_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn normalize_return_items(items: &[ReturnItemRequest]) -> Result<BTreeMap<i64, i64>, AppError> {
    if items.is_empty() {
        return Err(AppError::validation(
            "Izaberite bar jednu stavku za povrat.",
            serde_json::json!({ "field": "items" }),
        ));
    }

    let mut normalized = BTreeMap::<i64, i64>::new();
    for item in items {
        if item.sale_item_id <= 0 {
            return Err(AppError::validation(
                "Stavka računa nije ispravna.",
                serde_json::json!({ "field": "saleItemId" }),
            ));
        }

        if item.quantity_milli <= 0 {
            return Err(AppError::validation(
                "Količina za povrat mora biti pozitivna.",
                serde_json::json!({ "field": "quantityMilli" }),
            ));
        }

        let current = normalized.entry(item.sale_item_id).or_insert(0);
        *current = current.saturating_add(item.quantity_milli);
    }

    Ok(normalized)
}

fn proportional_unit_total(unit_price_minor: i64, quantity_milli: i64) -> i64 {
    unit_price_minor.saturating_mul(quantity_milli) / 1000
}

fn proportional_amount(amount_minor: i64, quantity_milli: i64, base_quantity_milli: i64) -> i64 {
    if base_quantity_milli <= 0 {
        return 0;
    }

    amount_minor.saturating_mul(quantity_milli) / base_quantity_milli
}

fn validate_receipt_id(receipt_id: i64) -> Result<(), AppError> {
    if receipt_id <= 0 {
        return Err(AppError::validation(
            "Račun nije ispravan.",
            serde_json::json!({ "field": "receiptId" }),
        ));
    }

    Ok(())
}

fn validate_user_id(user_id: i64) -> Result<(), AppError> {
    if user_id <= 0 {
        return Err(AppError::validation(
            "Korisnik nije ispravan.",
            serde_json::json!({ "field": "userId" }),
        ));
    }

    Ok(())
}

fn require_reason(value: &str, field: &str) -> Result<String, AppError> {
    normalized_optional_text(Some(value)).ok_or_else(|| {
        AppError::validation("Razlog je obavezan.", serde_json::json!({ "field": field }))
    })
}

fn validate_payment_method(value: &str) -> Result<(), AppError> {
    if matches!(value, "cash" | "card") {
        return Ok(());
    }

    Err(AppError::validation(
        "Način plaćanja nije ispravan.",
        serde_json::json!({ "field": "paymentMethod" }),
    ))
}

fn like_value(value: Option<&str>) -> Option<String> {
    normalized_optional_text(value).map(|value| format!("%{}%", value.to_lowercase()))
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_payment_methods(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::db::{test_database_path, Db};

    use super::{
        get_receipt_detail, return_items, search_receipts, void_receipt, ReceiptSearchQuery,
        ReturnItemRequest, ReturnItemsRequest, VoidReceiptRequest,
    };

    struct SeededReceipt {
        sale_id: i64,
        sale_item_id: i64,
        user_id: i64,
        product_id: i64,
    }

    fn with_receipt_database(test_name: &str, test: impl FnOnce(&Db, SeededReceipt)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            let seeded = seed_completed_receipt(&connection);
            test(&db, seeded);
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }

    fn seed_completed_receipt(connection: &Connection) -> SeededReceipt {
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('cashier', 'Mina Kasir', 'cashier', '2026-06-18T08:00:00Z', '2026-06-18T08:00:00Z')",
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
                 VALUES (?1, '2026-06-18T08:00:00Z', 0, 0, 'open', '2026-06-18T08:00:00Z', '2026-06-18T08:00:00Z')",
                params![user_id],
            )
            .expect("shift should insert");
        let shift_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES ('PDV 20', 2000, '2026-06-18T08:00:00Z', '2026-06-18T08:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        let tax_rate_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO products (
                    name,
                    sku,
                    barcode,
                    unit_of_measure,
                    sale_price_minor,
                    purchase_price_minor,
                    tax_rate_id,
                    minimum_stock_milli,
                    created_at,
                    updated_at
                 )
                 VALUES ('Kafa 200 g', 'KAFA-200', '8600000000010', 'kom', 50000, 32000, ?1, 1000, '2026-06-18T08:00:00Z', '2026-06-18T08:00:00Z')",
                params![tax_rate_id],
            )
            .expect("product should insert");
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                 VALUES (?1, 3000, '2026-06-18T08:05:00Z')",
                params![product_id],
            )
            .expect("inventory balance should insert");

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
                 VALUES ('R-2026-0001', ?1, ?2, 'completed', 'not_fiscalized', 100000, 0, 16667, 100000, '2026-06-18T09:15:00Z', '2026-06-18T09:15:00Z')",
                params![shift_id, user_id],
            )
            .expect("sale should insert");
        let sale_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_items (
                    sale_id,
                    product_id,
                    product_name,
                    product_sku,
                    product_barcode,
                    quantity_milli,
                    unit_price_minor,
                    discount_minor,
                    tax_rate_basis_points,
                    tax_minor,
                    total_minor
                 )
                 VALUES (?1, ?2, 'Kafa 200 g', 'KAFA-200', '8600000000010', 2000, 50000, 0, 2000, 16667, 100000)",
                params![sale_id, product_id],
            )
            .expect("sale item should insert");
        let sale_item_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', 100000, '2026-06-18T09:15:00Z')",
                params![sale_id],
            )
            .expect("payment should insert");

        connection
            .execute(
                "INSERT INTO inventory_movements (
                    product_id,
                    movement_type,
                    quantity_milli,
                    reason,
                    reference_type,
                    reference_id,
                    user_id,
                    created_at
                 )
                 VALUES (?1, 'sale', -2000, 'Prodaja', 'sale', ?2, ?3, '2026-06-18T09:15:00Z')",
                params![product_id, sale_id, user_id],
            )
            .expect("sale inventory movement should insert");

        SeededReceipt {
            sale_id,
            sale_item_id,
            user_id,
            product_id,
        }
    }

    fn balance_for(connection: &Connection, product_id: i64) -> i64 {
        connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                params![product_id],
                |row| row.get(0),
            )
            .expect("balance should query")
    }

    #[test]
    fn search_receipts_returns_completed_sale() {
        with_receipt_database("search_receipts_returns_completed_sale", |db, _seed| {
            let result = search_receipts(
                db,
                ReceiptSearchQuery {
                    receipt_number: Some("R-2026".to_string()),
                    product: Some("Kafa".to_string()),
                    ..ReceiptSearchQuery::default()
                },
            )
            .expect("receipt search should succeed");

            assert_eq!(result.receipts[0].receipt_number, "R-2026-0001");
        });
    }

    #[test]
    fn get_receipt_detail_includes_items_payments_and_action_flags() {
        with_receipt_database(
            "get_receipt_detail_includes_items_payments_and_action_flags",
            |db, seed| {
                let detail =
                    get_receipt_detail(db, seed.sale_id).expect("receipt detail should load");

                assert_eq!(detail.items[0].product_name, "Kafa 200 g");
                assert_eq!(detail.payments[0].payment_method, "cash");
                assert!(detail.can_void);
                assert!(detail.can_return);
            },
        );
    }

    #[test]
    fn void_receipt_creates_linked_document_and_restores_stock() {
        with_receipt_database(
            "void_receipt_creates_linked_document_and_restores_stock",
            |db, seed| {
                let detail = void_receipt(
                    db,
                    VoidReceiptRequest {
                        receipt_id: seed.sale_id,
                        user_id: seed.user_id,
                        reason: "Greska u unosu".to_string(),
                    },
                )
                .expect("receipt should void");

                let connection = db.open().expect("database should open");
                let movement_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM inventory_movements WHERE movement_type = 'void' AND reference_id = ?1",
                        params![detail.linked_documents[0].id],
                        |row| row.get(0),
                    )
                    .expect("movement count should query");

                assert_eq!(detail.status, "voided");
                assert_eq!(detail.linked_documents[0].document_type, "void");
                assert_eq!(balance_for(&connection, seed.product_id), 5000);
                assert_eq!(movement_count, 1);
            },
        );
    }

    #[test]
    fn void_receipt_rejects_duplicate_void() {
        with_receipt_database("void_receipt_rejects_duplicate_void", |db, seed| {
            void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seed.sale_id,
                    user_id: seed.user_id,
                    reason: "Greska u unosu".to_string(),
                },
            )
            .expect("first void should succeed");

            let error = void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seed.sale_id,
                    user_id: seed.user_id,
                    reason: "Ponovno storniranje".to_string(),
                },
            )
            .expect_err("duplicate void should fail");

            assert_eq!(error.code, "duplicate_void");
        });
    }

    #[test]
    fn return_items_rejects_excessive_quantity_without_inventory_change() {
        with_receipt_database(
            "return_items_rejects_excessive_quantity_without_inventory_change",
            |db, seed| {
                let error = return_items(
                    db,
                    ReturnItemsRequest {
                        receipt_id: seed.sale_id,
                        user_id: seed.user_id,
                        reason: "Kupac vratio previse".to_string(),
                        items: vec![ReturnItemRequest {
                            sale_item_id: seed.sale_item_id,
                            quantity_milli: 3000,
                        }],
                        refund_tender: None,
                    },
                )
                .expect_err("excessive return should fail");

                let connection = db.open().expect("database should open");
                let return_movements: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM inventory_movements WHERE movement_type = 'return'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("return movement count should query");

                assert_eq!(error.code, "return_quantity_exceeded");
                assert_eq!(balance_for(&connection, seed.product_id), 3000);
                assert_eq!(return_movements, 0);
            },
        );
    }

    #[test]
    fn return_items_creates_linked_document_for_selected_quantity() {
        with_receipt_database(
            "return_items_creates_linked_document_for_selected_quantity",
            |db, seed| {
                let detail = return_items(
                    db,
                    ReturnItemsRequest {
                        receipt_id: seed.sale_id,
                        user_id: seed.user_id,
                        reason: "Kupac vratio jedan komad".to_string(),
                        items: vec![ReturnItemRequest {
                            sale_item_id: seed.sale_item_id,
                            quantity_milli: 1000,
                        }],
                        refund_tender: None,
                    },
                )
                .expect("partial return should succeed");

                let connection = db.open().expect("database should open");

                assert_eq!(detail.status, "refunded");
                assert_eq!(detail.items[0].returned_quantity_milli, 1000);
                assert_eq!(detail.linked_documents[0].document_type, "return");
                assert_eq!(balance_for(&connection, seed.product_id), 4000);
            },
        );
    }

    #[test]
    fn void_records_negative_cash_payment_mirroring_original() {
        with_receipt_database("void_records_negative_cash_payment", |db, seeded| {
            void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska na racunu".to_string(),
                },
            )
            .expect("void should succeed");

            let connection = db.open().expect("database should open");
            let (void_sale_id, void_shift_id): (i64, i64) = connection
                .query_row(
                    "SELECT id, shift_id FROM sales
                     WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![seeded.sale_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("void document should exist");
            let refund: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments
                     WHERE sale_id = ?1 AND payment_method = 'cash'",
                    params![void_sale_id],
                    |row| row.get(0),
                )
                .expect("mirrored cash refund should exist");
            assert_eq!(refund, -100_000);

            let open_shift: i64 = connection
                .query_row(
                    "SELECT id FROM shifts WHERE status = 'open'
                     ORDER BY opened_at DESC, id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("open shift should exist");
            assert_eq!(void_shift_id, open_shift);
        });
    }

    #[test]
    fn void_mirrors_split_cash_and_card_tenders() {
        with_receipt_database("void_mirrors_split_tenders", |db, seeded| {
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO sales (
                        local_receipt_number, shift_id, cashier_id, status, fiscal_status,
                        subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                     SELECT 'R-2026-0002', shift_id, cashier_id, 'completed', 'not_fiscalized',
                        100000, 0, 16667, 100000, '2026-06-18T09:30:00Z', '2026-06-18T09:30:00Z'
                     FROM sales WHERE id = ?1",
                    params![seeded.sale_id],
                )
                .expect("second sale should insert");
            let sale2 = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO sale_items (
                        sale_id, product_id, product_name, product_sku, product_barcode,
                        quantity_milli, unit_price_minor, discount_minor, tax_rate_basis_points,
                        tax_minor, total_minor)
                     VALUES (?1, ?2, 'Kafa 200 g', 'KAFA-200', '8600000000010', 2000, 50000, 0,
                        2000, 16667, 100000)",
                    params![sale2, seeded.product_id],
                )
                .expect("second sale item should insert");
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (?1, 'cash', 60000, '2026-06-18T09:30:00Z')",
                    params![sale2],
                )
                .expect("cash payment should insert");
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (?1, 'card', 40000, '2026-06-18T09:30:00Z')",
                    params![sale2],
                )
                .expect("card payment should insert");

            void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: sale2,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect("void should succeed");

            let void_id: i64 = connection
                .query_row(
                    "SELECT id FROM sales WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![sale2],
                    |row| row.get(0),
                )
                .expect("void document should exist");
            let cash: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments WHERE sale_id = ?1 AND payment_method = 'cash'",
                    params![void_id],
                    |row| row.get(0),
                )
                .expect("cash refund should exist");
            let card: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments WHERE sale_id = ?1 AND payment_method = 'card'",
                    params![void_id],
                    |row| row.get(0),
                )
                .expect("card refund should exist");
            assert_eq!(cash, -60_000);
            assert_eq!(card, -40_000);
        });
    }

    #[test]
    fn void_requires_open_shift() {
        with_receipt_database("void_requires_open_shift", |db, seeded| {
            db.open()
                .expect("database should open")
                .execute("UPDATE shifts SET status = 'closed'", [])
                .expect("shift should close");

            let error = void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect_err("void without an open shift should fail");
            assert_eq!(error.code, "shift_required");

            let voids: i64 = db
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT COUNT(*) FROM sales WHERE document_type = 'void'",
                    [],
                    |row| row.get(0),
                )
                .expect("count should query");
            assert_eq!(voids, 0);
        });
    }

    #[test]
    fn return_records_negative_cash_refund_by_default() {
        with_receipt_database("return_default_cash_refund", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen artikal".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect("return should succeed");

            let connection = db.open().expect("database should open");
            let refund: i64 = connection
                .query_row(
                    "SELECT sp.amount_minor FROM sale_payments sp
                     JOIN sales s ON s.id = sp.sale_id
                     WHERE s.original_sale_id = ?1 AND s.document_type = 'return'
                       AND sp.payment_method = 'cash'",
                    params![seeded.sale_id],
                    |row| row.get(0),
                )
                .expect("cash refund should exist");
            assert_eq!(refund, -50_000);
        });
    }

    #[test]
    fn return_refund_tender_card_records_negative_card_refund() {
        with_receipt_database("return_card_refund", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Zamena velicine".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: Some("card".to_string()),
                },
            )
            .expect("return should succeed");

            let connection = db.open().expect("database should open");
            let refund: i64 = connection
                .query_row(
                    "SELECT sp.amount_minor FROM sale_payments sp
                     JOIN sales s ON s.id = sp.sale_id
                     WHERE s.original_sale_id = ?1 AND s.document_type = 'return'
                       AND sp.payment_method = 'card'",
                    params![seeded.sale_id],
                    |row| row.get(0),
                )
                .expect("card refund should exist");
            assert_eq!(refund, -50_000);
        });
    }

    #[test]
    fn return_requires_open_shift() {
        with_receipt_database("return_requires_open_shift", |db, seeded| {
            db.open()
                .expect("database should open")
                .execute("UPDATE shifts SET status = 'closed'", [])
                .expect("shift should close");

            let error = return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect_err("return without an open shift should fail");
            assert_eq!(error.code, "shift_required");
        });
    }

    #[test]
    fn return_rejects_invalid_refund_tender() {
        with_receipt_database("return_invalid_tender", |db, seeded| {
            let error = return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: Some("bitcoin".to_string()),
                },
            )
            .expect_err("invalid refund tender should fail");
            assert_eq!(error.code, "validation_error");
        });
    }

    #[test]
    fn void_is_blocked_after_a_partial_return() {
        with_receipt_database("void_blocked_after_return", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Delimican povrat".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect("partial return should succeed");

            let error = void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect_err("void after a return must stay blocked");
            assert_eq!(error.code, "invalid_receipt_state");
        });
    }
}
