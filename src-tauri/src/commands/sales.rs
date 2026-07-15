use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::inventory::{write_stock_movement, StockMovementWrite};
use crate::commands::settings::{
    ReceiptSettings, SalesSettings, RECEIPT_SETTINGS_KEY, SALES_SETTINGS_KEY,
};
use crate::db::Db;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleDraftItem {
    pub product_id: i64,
    pub quantity_milli: i64,
    pub discount: Option<DiscountRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleDraftRequest {
    pub items: Vec<SaleDraftItem>,
    pub receipt_discount: Option<DiscountRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DiscountRequest {
    #[serde(rename = "amount", rename_all = "camelCase")]
    Amount { amount_minor: i64 },
    #[serde(rename = "percent", rename_all = "camelCase")]
    Percent { basis_points: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentMethod {
    Cash,
    Card,
}

impl PaymentMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Card => "card",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDraft {
    pub method: PaymentMethod,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSaleRequest {
    pub items: Vec<SaleDraftItem>,
    pub receipt_discount: Option<DiscountRequest>,
    pub payments: Vec<PaymentDraft>,
    #[serde(default)]
    pub allow_stock_override: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalePreviewItem {
    pub product_id: i64,
    pub product_name: String,
    pub product_sku: String,
    pub product_barcode: Option<String>,
    pub quantity_milli: i64,
    pub quantity_label: String,
    pub unit_price_minor: i64,
    pub discount_minor: i64,
    pub tax_rate_basis_points: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalePreview {
    pub items: Vec<SalePreviewItem>,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedSale {
    pub id: i64,
    pub local_receipt_number: String,
    pub created_at: String,
    pub cashier_name: String,
    pub fiscal_status: String,
    pub items: Vec<SalePreviewItem>,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub payments: Vec<PaymentDraft>,
    pub cash_received_minor: i64,
    pub change_due_minor: i64,
}

#[derive(Debug, Clone)]
struct SaleProduct {
    id: i64,
    name: String,
    sku: String,
    barcode: Option<String>,
    unit_of_measure: String,
    sale_price_minor: i64,
    tax_rate_basis_points: i64,
    current_stock_milli: i64,
    allow_negative_stock: bool,
    active: bool,
}

#[derive(Debug, Clone)]
struct ComputedLine {
    product: SaleProduct,
    quantity_milli: i64,
    unit_price_minor: i64,
    discount_minor: i64,
    tax_rate_basis_points: i64,
    tax_minor: i64,
    total_minor: i64,
}

#[derive(Debug, Clone)]
struct SaleComputation {
    lines: Vec<ComputedLine>,
    preview: SalePreview,
}

#[derive(Debug, Clone)]
struct OpenShift {
    id: i64,
    cashier_id: i64,
    cashier_name: String,
}

#[derive(Debug, Clone)]
struct PaymentAllocation {
    cash_received_minor: i64,
    change_due_minor: i64,
    stored_cash_minor: i64,
    stored_card_minor: i64,
}

#[tauri::command]
pub fn sales_preview(
    state: State<'_, AppState>,
    request: SaleDraftRequest,
) -> Result<SalePreview, CommandError> {
    build_sale_preview(state.db(), request).map_err(Into::into)
}

#[tauri::command]
pub fn sales_complete(
    state: State<'_, AppState>,
    request: CompleteSaleRequest,
) -> Result<CompletedSale, CommandError> {
    complete_sale_transaction(state.db(), request).map_err(Into::into)
}

pub fn build_sale_preview(db: &Db, request: SaleDraftRequest) -> Result<SalePreview, AppError> {
    let connection = db.open()?;
    let computation = compute_sale(&connection, &request)?;

    Ok(computation.preview)
}

pub fn complete_sale_transaction(
    db: &Db,
    request: CompleteSaleRequest,
) -> Result<CompletedSale, AppError> {
    let mut connection = db.open()?;
    let tx = connection.transaction()?;
    let CompleteSaleRequest {
        items,
        receipt_discount,
        payments,
        allow_stock_override,
    } = request;
    let draft = SaleDraftRequest {
        items,
        receipt_discount,
    };
    let shift = load_open_shift(&tx)?;
    let computation = compute_sale(&tx, &draft)?;
    let overselling = load_allow_overselling(&tx)? || allow_stock_override.unwrap_or(false);
    validate_stock(&computation.lines, overselling)?;
    let payment = validate_payments(&payments, computation.preview.total_minor)?;
    let created_at = current_timestamp(&tx)?;
    let local_receipt_number = take_next_receipt_number(&tx, &created_at)?;

    tx.execute(
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
         VALUES (?1, ?2, ?3, 'completed', 'not_fiscalized', ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            local_receipt_number,
            shift.id,
            shift.cashier_id,
            computation.preview.subtotal_minor,
            computation.preview.discount_minor,
            computation.preview.tax_minor,
            computation.preview.total_minor,
            created_at
        ],
    )?;
    let sale_id = tx.last_insert_rowid();

    for line in &computation.lines {
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
                total_minor
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                sale_id,
                line.product.id,
                line.product.name,
                line.product.sku,
                line.product.barcode,
                line.quantity_milli,
                line.unit_price_minor,
                line.discount_minor,
                line.tax_rate_basis_points,
                line.tax_minor,
                line.total_minor
            ],
        )?;

        write_stock_movement(
            &tx,
            StockMovementWrite {
                product_id: line.product.id,
                movement_type: "sale",
                quantity_milli: -line.quantity_milli,
                reason: Some("Prodaja"),
                reference_type: Some("sale"),
                reference_id: Some(sale_id),
                user_id: Some(shift.cashier_id),
                created_at: &created_at,
                allow_overselling: overselling,
            },
        )?;
    }

    insert_payment_if_present(
        &tx,
        sale_id,
        PaymentMethod::Cash,
        payment.stored_cash_minor,
        &created_at,
    )?;
    insert_payment_if_present(
        &tx,
        sale_id,
        PaymentMethod::Card,
        payment.stored_card_minor,
        &created_at,
    )?;
    tx.execute(
        "UPDATE shifts
         SET expected_cash_minor = expected_cash_minor + ?1,
             updated_at = ?2
         WHERE id = ?3",
        params![payment.stored_cash_minor, created_at, shift.id],
    )?;

    let preview = computation.preview;
    let completed = CompletedSale {
        id: sale_id,
        local_receipt_number,
        created_at,
        cashier_name: shift.cashier_name,
        fiscal_status: "not_fiscalized".to_string(),
        items: preview.items,
        subtotal_minor: preview.subtotal_minor,
        discount_minor: preview.discount_minor,
        tax_minor: preview.tax_minor,
        total_minor: preview.total_minor,
        payments: stored_payments(&payment),
        cash_received_minor: payment.cash_received_minor,
        change_due_minor: payment.change_due_minor,
    };

    tx.commit()?;
    Ok(completed)
}

fn compute_sale(
    connection: &Connection,
    request: &SaleDraftRequest,
) -> Result<SaleComputation, AppError> {
    if request.items.is_empty() {
        return Err(AppError::business("empty_cart", "Korpa je prazna."));
    }

    let mut lines = Vec::with_capacity(request.items.len());

    for item in &request.items {
        if item.quantity_milli <= 0 {
            return Err(AppError::business(
                "invalid_quantity",
                "Kolicina mora biti veca od nule.",
            ));
        }

        let product = load_sale_product(connection, item.product_id)?;

        if !product.active {
            return Err(AppError::business(
                "inactive_product",
                "Artikal nije aktivan za prodaju.",
            ));
        }

        let line_gross = rounded_div(
            product.sale_price_minor as i128 * item.quantity_milli as i128,
            1000,
        )?;
        let discount_minor = discount_amount(line_gross, item.discount.as_ref())?;
        let total_minor = line_gross - discount_minor;
        let tax_minor = included_tax(total_minor, product.tax_rate_basis_points)?;
        let unit_price_minor = product.sale_price_minor;
        let tax_rate_basis_points = product.tax_rate_basis_points;

        lines.push(ComputedLine {
            product,
            quantity_milli: item.quantity_milli,
            unit_price_minor,
            discount_minor,
            tax_rate_basis_points,
            tax_minor,
            total_minor,
        });
    }

    let subtotal_minor = lines.iter().try_fold(0_i64, |sum, line| {
        checked_add(sum, line.unit_price_minor * line.quantity_milli / 1000)
    })?;
    let item_discount_minor = lines
        .iter()
        .try_fold(0_i64, |sum, line| checked_add(sum, line.discount_minor))?;
    let line_total_minor = lines
        .iter()
        .try_fold(0_i64, |sum, line| checked_add(sum, line.total_minor))?;
    let receipt_discount_minor =
        discount_amount(line_total_minor, request.receipt_discount.as_ref())?;
    let total_minor = line_total_minor - receipt_discount_minor;
    let tax_minor = sale_tax_after_receipt_discount(&lines, receipt_discount_minor)?;
    let items = lines
        .iter()
        .map(|line| SalePreviewItem {
            product_id: line.product.id,
            product_name: line.product.name.clone(),
            product_sku: line.product.sku.clone(),
            product_barcode: line.product.barcode.clone(),
            quantity_milli: line.quantity_milli,
            quantity_label: quantity_label(line.quantity_milli, &line.product.unit_of_measure),
            unit_price_minor: line.product.sale_price_minor,
            discount_minor: line.discount_minor,
            tax_rate_basis_points: line.tax_rate_basis_points,
            tax_minor: line.tax_minor,
            total_minor: line.total_minor,
        })
        .collect();

    Ok(SaleComputation {
        lines,
        preview: SalePreview {
            items,
            subtotal_minor,
            discount_minor: item_discount_minor + receipt_discount_minor,
            tax_minor,
            total_minor,
        },
    })
}

fn load_sale_product(connection: &Connection, product_id: i64) -> Result<SaleProduct, AppError> {
    connection
        .query_row(
            "SELECT
                p.id,
                p.name,
                p.sku,
                p.barcode,
                p.unit_of_measure,
                p.sale_price_minor,
                tr.rate_basis_points,
                COALESCE(ib.quantity_milli, 0),
                p.allow_negative_stock,
                p.active
             FROM products p
             JOIN tax_rates tr ON tr.id = p.tax_rate_id
             LEFT JOIN inventory_balances ib ON ib.product_id = p.id
             WHERE p.id = ?1",
            params![product_id],
            |row| {
                let allow_negative_stock: i64 = row.get(8)?;
                let active: i64 = row.get(9)?;

                Ok(SaleProduct {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sku: row.get(2)?,
                    barcode: row.get(3)?,
                    unit_of_measure: row.get(4)?,
                    sale_price_minor: row.get(5)?,
                    tax_rate_basis_points: row.get(6)?,
                    current_stock_milli: row.get(7)?,
                    allow_negative_stock: allow_negative_stock == 1,
                    active: active == 1,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Artikal nije pronadjen."))
}

fn load_open_shift(connection: &Connection) -> Result<OpenShift, AppError> {
    connection
        .query_row(
            "SELECT s.id, u.id, u.display_name
             FROM shifts s
             JOIN users u ON u.id = s.user_id
             WHERE s.status = 'open'
             ORDER BY s.opened_at DESC, s.id DESC
             LIMIT 1",
            [],
            |row| {
                Ok(OpenShift {
                    id: row.get(0)?,
                    cashier_id: row.get(1)?,
                    cashier_name: row.get(2)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::business("shift_required", "Smena nije otvorena."))
}

fn load_allow_overselling(connection: &Connection) -> Result<bool, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![SALES_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    match stored {
        Some(value) => Ok(serde_json::from_str::<SalesSettings>(&value)
            .map(|settings| settings.allow_overselling)
            .unwrap_or(false)),
        None => Ok(false),
    }
}

fn validate_stock(lines: &[ComputedLine], allow_overselling: bool) -> Result<(), AppError> {
    let mut required_by_product: HashMap<i64, (i64, i64, bool)> = HashMap::new();

    for line in lines {
        let entry = required_by_product.entry(line.product.id).or_insert((
            0,
            line.product.current_stock_milli,
            line.product.allow_negative_stock,
        ));
        entry.0 = checked_add(entry.0, line.quantity_milli)?;
    }

    for (product_id, (required, current, allow_negative)) in required_by_product {
        if !allow_negative && !allow_overselling && current < required {
            return Err(AppError::business_with_details(
                "insufficient_stock",
                "Nema dovoljno zaliha.",
                serde_json::json!({
                    "productId": product_id,
                    "currentStockMilli": current,
                    "requiredQuantityMilli": required
                }),
            ));
        }
    }

    Ok(())
}

fn validate_payments(
    payments: &[PaymentDraft],
    total_minor: i64,
) -> Result<PaymentAllocation, AppError> {
    let mut cash_received_minor = 0_i64;
    let mut card_minor = 0_i64;

    for payment in payments {
        if payment.amount_minor < 0 {
            return Err(AppError::business(
                "payment_mismatch",
                "Placanja se ne poklapaju sa ukupnim iznosom.",
            ));
        }

        match payment.method {
            PaymentMethod::Cash => {
                cash_received_minor = checked_add(cash_received_minor, payment.amount_minor)?;
            }
            PaymentMethod::Card => {
                card_minor = checked_add(card_minor, payment.amount_minor)?;
            }
        }
    }

    let tendered_minor = checked_add(cash_received_minor, card_minor)?;

    if card_minor > total_minor || tendered_minor < total_minor {
        return Err(AppError::business(
            "payment_mismatch",
            "Placanja se ne poklapaju sa ukupnim iznosom.",
        ));
    }

    Ok(PaymentAllocation {
        cash_received_minor,
        change_due_minor: tendered_minor - total_minor,
        stored_cash_minor: total_minor - card_minor,
        stored_card_minor: card_minor,
    })
}

fn insert_payment_if_present(
    connection: &Connection,
    sale_id: i64,
    method: PaymentMethod,
    amount_minor: i64,
    created_at: &str,
) -> Result<(), AppError> {
    if amount_minor == 0 {
        return Ok(());
    }

    connection.execute(
        "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![sale_id, method.as_str(), amount_minor, created_at],
    )?;
    Ok(())
}

fn stored_payments(payment: &PaymentAllocation) -> Vec<PaymentDraft> {
    let mut payments = Vec::new();

    if payment.stored_cash_minor > 0 {
        payments.push(PaymentDraft {
            method: PaymentMethod::Cash,
            amount_minor: payment.stored_cash_minor,
        });
    }

    if payment.stored_card_minor > 0 {
        payments.push(PaymentDraft {
            method: PaymentMethod::Card,
            amount_minor: payment.stored_card_minor,
        });
    }

    payments
}

fn take_next_receipt_number(connection: &Connection, updated_at: &str) -> Result<String, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![RECEIPT_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    let settings = match stored {
        Some(value) => serde_json::from_str::<ReceiptSettings>(&value).map_err(|source| {
            AppError::InvalidState(format!("Podesavanja racuna nisu ispravna: {source}"))
        })?,
        None => ReceiptSettings::default(),
    };
    let receipt_number = format!("{}{:06}", settings.prefix, settings.next_sequence_number);
    let next_settings = ReceiptSettings {
        next_sequence_number: settings.next_sequence_number + 1,
        ..settings
    };
    let value_json = serde_json::to_string(&next_settings).map_err(|source| {
        AppError::InvalidState(format!("Podesavanja racuna nisu ispravna: {source}"))
    })?;

    connection.execute(
        "INSERT INTO settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
             value_json = excluded.value_json,
             updated_at = excluded.updated_at",
        params![RECEIPT_SETTINGS_KEY, value_json, updated_at],
    )?;

    Ok(receipt_number)
}

fn current_timestamp(connection: &Connection) -> Result<String, AppError> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(Into::into)
}

fn discount_amount(total_minor: i64, discount: Option<&DiscountRequest>) -> Result<i64, AppError> {
    let Some(discount) = discount else {
        return Ok(0);
    };

    let amount = match discount {
        DiscountRequest::Amount { amount_minor } => {
            if *amount_minor < 0 {
                return Err(AppError::business(
                    "invalid_discount",
                    "Popust mora biti pozitivan.",
                ));
            }

            *amount_minor
        }
        DiscountRequest::Percent { basis_points } => {
            if !(0..=10_000).contains(basis_points) {
                return Err(AppError::business(
                    "invalid_discount",
                    "Procenat popusta mora biti izmedju 0 i 100%.",
                ));
            }

            rounded_div(total_minor as i128 * *basis_points as i128, 10_000)?
        }
    };

    if amount > total_minor {
        return Err(AppError::business(
            "invalid_discount",
            "Popust ne moze biti veci od iznosa.",
        ));
    }

    Ok(amount)
}

fn sale_tax_after_receipt_discount(
    lines: &[ComputedLine],
    receipt_discount_minor: i64,
) -> Result<i64, AppError> {
    let line_total_minor = lines
        .iter()
        .try_fold(0_i64, |sum, line| checked_add(sum, line.total_minor))?;

    if receipt_discount_minor == 0 {
        return lines
            .iter()
            .try_fold(0_i64, |sum, line| checked_add(sum, line.tax_minor));
    }

    if line_total_minor == 0 {
        return Ok(0);
    }

    let mut allocated_discount = 0_i64;
    let mut tax_total = 0_i64;

    for (index, line) in lines.iter().enumerate() {
        let line_receipt_discount = if index == lines.len() - 1 {
            receipt_discount_minor - allocated_discount
        } else {
            let allocated = rounded_div(
                line.total_minor as i128 * receipt_discount_minor as i128,
                line_total_minor as i128,
            )?;
            allocated_discount = checked_add(allocated_discount, allocated)?;
            allocated
        };
        let taxable_line_total = line.total_minor - line_receipt_discount;
        tax_total = checked_add(
            tax_total,
            included_tax(taxable_line_total, line.tax_rate_basis_points)?,
        )?;
    }

    Ok(tax_total)
}

fn included_tax(total_minor: i64, rate_basis_points: i64) -> Result<i64, AppError> {
    if rate_basis_points < 0 {
        return Err(AppError::InvalidState(
            "PDV stopa artikla nije ispravna.".to_string(),
        ));
    }

    if rate_basis_points == 0 || total_minor == 0 {
        return Ok(0);
    }

    rounded_div(
        total_minor as i128 * rate_basis_points as i128,
        (10_000 + rate_basis_points) as i128,
    )
}

fn rounded_div(numerator: i128, denominator: i128) -> Result<i64, AppError> {
    if denominator <= 0 || numerator < 0 {
        return Err(AppError::InvalidState(
            "Obracun prodaje nije ispravan.".to_string(),
        ));
    }

    let value = (numerator + denominator / 2) / denominator;
    i64::try_from(value)
        .map_err(|_| AppError::InvalidState("Obracun prodaje je prevelik za upis.".to_string()))
}

fn checked_add(left: i64, right: i64) -> Result<i64, AppError> {
    left.checked_add(right)
        .ok_or_else(|| AppError::InvalidState("Obracun prodaje je prevelik za upis.".to_string()))
}

fn quantity_label(quantity_milli: i64, unit: &str) -> String {
    if quantity_milli % 1000 == 0 {
        format!("{} {unit}", quantity_milli / 1000)
    } else {
        let whole = quantity_milli / 1000;
        let fraction = (quantity_milli % 1000).abs();
        format!("{whole}.{fraction:03} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::commands::sales::{
        build_sale_preview, complete_sale_transaction, CompleteSaleRequest, DiscountRequest,
        PaymentDraft, PaymentMethod, SaleDraftItem, SaleDraftRequest,
    };
    use crate::db::{test_database_path, Db};

    struct SeededSaleData {
        db_path: std::path::PathBuf,
        db: Db,
        product_id: i64,
    }

    fn seed_sale_data(quantity_milli: i64, open_shift: bool) -> SeededSaleData {
        let db_path = test_database_path("seed_sale_data");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let connection = db.open().expect("database should open");

        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('cashier', 'Kasir', 'cashier', '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();

        if open_shift {
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
                     VALUES (?1, '2026-06-18T09:00:00Z', 0, 0, 'open', '2026-06-18T09:00:00Z', '2026-06-18T09:00:00Z')",
                    params![cashier_id],
                )
                .expect("shift should insert");
        }

        connection
            .execute(
                "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES ('PDV 20', 2000, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
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
                    allow_negative_stock,
                    active,
                    created_at,
                    updated_at
                 )
                 VALUES ('Mleko 1 l', 'MLEKO-1L', '8600000000010', 'kom', 12000, 9000, ?1, 0, 0, 1, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                params![tax_rate_id],
            )
            .expect("product should insert");
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                 VALUES (?1, ?2, '2026-06-18T10:00:00Z')",
                params![product_id, quantity_milli],
            )
            .expect("inventory balance should insert");

        SeededSaleData {
            db_path,
            db,
            product_id,
        }
    }

    fn sale_draft(product_id: i64, quantity_milli: i64) -> SaleDraftRequest {
        SaleDraftRequest {
            items: vec![SaleDraftItem {
                product_id,
                quantity_milli,
                discount: None,
            }],
            receipt_discount: None,
        }
    }

    #[test]
    fn build_sale_preview_recalculates_totals_from_database_prices() {
        let seeded = seed_sale_data(5000, true);

        let preview = build_sale_preview(&seeded.db, sale_draft(seeded.product_id, 2000))
            .expect("preview should build");

        assert_eq!(preview.subtotal_minor, 24000);
        assert_eq!(preview.discount_minor, 0);
        assert_eq!(preview.tax_minor, 4000);
        assert_eq!(preview.total_minor, 24000);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_inserts_sale_and_decrements_stock() {
        let seeded = seed_sale_data(5000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: Some(DiscountRequest::Amount { amount_minor: 0 }),
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 30000,
            }],
            allow_stock_override: None,
        };

        let completed =
            complete_sale_transaction(&seeded.db, request).expect("sale should complete");

        assert_eq!(completed.local_receipt_number, "VP-000001");
        assert_eq!(completed.total_minor, 24000);
        assert_eq!(completed.cash_received_minor, 30000);
        assert_eq!(completed.change_due_minor, 6000);

        let connection = seeded.db.open().expect("database should open");
        let balance: i64 = connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                params![seeded.product_id],
                |row| row.get(0),
            )
            .expect("balance should query");
        let stored_cash_payment: i64 = connection
            .query_row("SELECT amount_minor FROM sale_payments", [], |row| {
                row.get(0)
            })
            .expect("payment should query");
        let movement_quantity: i64 = connection
            .query_row(
                "SELECT quantity_milli FROM inventory_movements",
                [],
                |row| row.get(0),
            )
            .expect("movement should query");

        assert_eq!(balance, 3000);
        assert_eq!(stored_cash_payment, 24000);
        assert_eq!(movement_quantity, -2000);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_requires_open_shift() {
        let seeded = seed_sale_data(5000, false);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 1000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 12000,
            }],
            allow_stock_override: None,
        };

        let error = complete_sale_transaction(&seeded.db, request)
            .expect_err("sale without open shift should fail");

        assert_eq!(error.code(), "shift_required");

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_rolls_back_when_stock_is_insufficient() {
        let seeded = seed_sale_data(1000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 24000,
            }],
            allow_stock_override: None,
        };

        let error = complete_sale_transaction(&seeded.db, request)
            .expect_err("insufficient stock should fail");

        assert_eq!(error.code(), "insufficient_stock");

        let connection = seeded.db.open().expect("database should open");
        let sale_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sales", [], |row| row.get(0))
            .expect("sale count should query");
        let balance: i64 = connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                params![seeded.product_id],
                |row| row.get(0),
            )
            .expect("balance should query");

        assert_eq!(sale_count, 0);
        assert_eq!(balance, 1000);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_allows_oversell_with_per_sale_override() {
        let seeded = seed_sale_data(1000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 24000,
            }],
            allow_stock_override: Some(true),
        };

        complete_sale_transaction(&seeded.db, request).expect("oversell override should succeed");

        let connection = seeded.db.open().expect("database should open");
        let balance: i64 = connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                params![seeded.product_id],
                |row| row.get(0),
            )
            .expect("balance should query");
        let sale_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sales", [], |row| row.get(0))
            .expect("sale count should query");

        assert_eq!(balance, -1000);
        assert_eq!(sale_count, 1);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_allows_oversell_when_shop_setting_enabled() {
        let seeded = seed_sale_data(1000, true);
        seeded
            .db
            .open()
            .expect("database should open")
            .execute(
                "INSERT INTO settings (key, value_json, updated_at)
                 VALUES ('sales', '{\"allowOverselling\":true}', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("sales setting should insert");

        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 24000,
            }],
            allow_stock_override: None,
        };

        complete_sale_transaction(&seeded.db, request)
            .expect("shop-enabled oversell should succeed");

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_still_blocks_oversell_by_default() {
        let seeded = seed_sale_data(1000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 24000,
            }],
            allow_stock_override: None,
        };

        let error = complete_sale_transaction(&seeded.db, request)
            .expect_err("default should still block oversell");
        assert_eq!(error.code(), "insufficient_stock");

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_decrements_repeated_product_lines_consistently() {
        let seeded = seed_sale_data(5000, true);
        let request = CompleteSaleRequest {
            items: vec![
                SaleDraftItem {
                    product_id: seeded.product_id,
                    quantity_milli: 2000,
                    discount: None,
                },
                SaleDraftItem {
                    product_id: seeded.product_id,
                    quantity_milli: 1000,
                    discount: None,
                },
            ],
            receipt_discount: Some(DiscountRequest::Amount { amount_minor: 0 }),
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: 40000,
            }],
            allow_stock_override: None,
        };

        complete_sale_transaction(&seeded.db, request).expect("sale should complete");

        let connection = seeded.db.open().expect("database should open");
        let balance: i64 = connection
            .query_row(
                "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                params![seeded.product_id],
                |row| row.get(0),
            )
            .expect("balance should query");
        assert_eq!(balance, 2000);

        let movement_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM inventory_movements WHERE product_id = ?1",
                params![seeded.product_id],
                |row| row.get(0),
            )
            .expect("movement count should query");
        assert_eq!(movement_count, 2);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_accepts_matching_mixed_payment() {
        let seeded = seed_sale_data(5000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![
                PaymentDraft {
                    method: PaymentMethod::Cash,
                    amount_minor: 10000,
                },
                PaymentDraft {
                    method: PaymentMethod::Card,
                    amount_minor: 14000,
                },
            ],
            allow_stock_override: None,
        };

        let completed =
            complete_sale_transaction(&seeded.db, request).expect("sale should complete");

        assert_eq!(completed.total_minor, 24000);
        assert_eq!(completed.change_due_minor, 0);

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn complete_sale_transaction_rejects_payment_mismatch() {
        let seeded = seed_sale_data(5000, true);
        let request = CompleteSaleRequest {
            items: sale_draft(seeded.product_id, 2000).items,
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Card,
                amount_minor: 25000,
            }],
            allow_stock_override: None,
        };

        let error = complete_sale_transaction(&seeded.db, request)
            .expect_err("card overpayment should fail");

        assert_eq!(error.code(), "payment_mismatch");

        let _ = std::fs::remove_file(seeded.db_path);
    }
}
