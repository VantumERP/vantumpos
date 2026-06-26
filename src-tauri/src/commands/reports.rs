use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDateQuery {
    pub from: String,
    pub to: String,
    pub shift_id: Option<i64>,
    pub cashier_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSalesQuery {
    pub from: String,
    pub to: String,
    pub category_id: Option<i64>,
    pub product_id: Option<i64>,
}

impl ProductSalesQuery {
    fn date_query(&self) -> ReportDateQuery {
        ReportDateQuery {
            from: self.from.clone(),
            to: self.to.clone(),
            shift_id: None,
            cashier_id: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportReportType {
    DailyTurnover,
    ShiftTurnover,
    CashierTurnover,
    PaymentMethods,
    ProductSales,
    CategorySales,
    LowStock,
}

impl ExportReportType {
    fn file_slug(&self) -> &'static str {
        match self {
            Self::DailyTurnover => "dnevni-promet",
            Self::ShiftTurnover => "promet-po-smenama",
            Self::CashierTurnover => "promet-po-kasirima",
            Self::PaymentMethods => "placanja",
            Self::ProductSales => "prodaja-artikala",
            Self::CategorySales => "prodaja-kategorija",
            Self::LowStock => "nizak-lager",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReportRequest {
    pub report_type: ExportReportType,
    pub query: ProductSalesQuery,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTurnoverSummary {
    pub total_minor: i64,
    pub cash_minor: i64,
    pub card_minor: i64,
    pub receipt_count: i64,
    pub average_receipt_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTurnoverRow {
    pub day: String,
    pub receipt_count: i64,
    pub cash_minor: i64,
    pub card_minor: i64,
    pub total_minor: i64,
    pub refunds_or_voids_minor: i64,
    pub refunds_or_voids_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTurnoverReport {
    pub summary: DailyTurnoverSummary,
    pub rows: Vec<DailyTurnoverRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftTurnoverRow {
    pub shift_id: i64,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub cashier_name: String,
    pub receipt_count: i64,
    pub cash_minor: i64,
    pub card_minor: i64,
    pub total_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftTurnoverReport {
    pub rows: Vec<ShiftTurnoverRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftListItem {
    pub id: i64,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub cashier_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierTurnoverRow {
    pub cashier_id: i64,
    pub cashier_name: String,
    pub receipt_count: i64,
    pub total_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierTurnoverReport {
    pub rows: Vec<CashierTurnoverRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodRow {
    pub payment_method: String,
    pub receipt_count: i64,
    pub total_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodReport {
    pub rows: Vec<PaymentMethodRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSalesRow {
    pub product_id: Option<i64>,
    pub product_name: String,
    pub product_sku: String,
    pub quantity_milli: i64,
    pub revenue_minor: i64,
    pub discount_minor: i64,
    pub estimated_margin_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSalesReport {
    pub rows: Vec<ProductSalesRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySalesRow {
    pub category_id: Option<i64>,
    pub category_name: String,
    pub quantity_milli: i64,
    pub revenue_minor: i64,
    pub discount_minor: i64,
    pub estimated_margin_minor: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySalesReport {
    pub rows: Vec<CategorySalesRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LowStockRow {
    pub product_id: i64,
    pub product_name: String,
    pub product_sku: String,
    pub current_stock_milli: i64,
    pub minimum_stock_milli: i64,
    pub difference_milli: i64,
    pub last_movement_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LowStockReport {
    pub rows: Vec<LowStockRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    pub file_name: String,
    pub path: String,
    pub mime_type: &'static str,
    pub row_count: usize,
}

#[tauri::command]
pub fn reports_daily_turnover(
    state: State<'_, AppState>,
    query: ReportDateQuery,
) -> Result<DailyTurnoverReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_daily_turnover(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_shift_turnover(
    state: State<'_, AppState>,
    query: ReportDateQuery,
) -> Result<ShiftTurnoverReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_shift_turnover(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_cashier_turnover(
    state: State<'_, AppState>,
    query: ReportDateQuery,
) -> Result<CashierTurnoverReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_cashier_turnover(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_payment_methods(
    state: State<'_, AppState>,
    query: ReportDateQuery,
) -> Result<PaymentMethodReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_payment_methods(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_product_sales(
    state: State<'_, AppState>,
    query: ProductSalesQuery,
) -> Result<ProductSalesReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_product_sales(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_category_sales(
    state: State<'_, AppState>,
    query: ReportDateQuery,
) -> Result<CategorySalesReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_category_sales(&connection, &query).map_err(Into::into)
}

#[tauri::command]
pub fn reports_low_stock(state: State<'_, AppState>) -> Result<LowStockReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_low_stock(&connection).map_err(Into::into)
}

#[tauri::command]
pub fn reports_list_shifts(state: State<'_, AppState>) -> Result<Vec<ShiftListItem>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    query_list_shifts(&connection).map_err(Into::into)
}

#[tauri::command]
pub fn reports_export_csv(
    state: State<'_, AppState>,
    request: ExportReportRequest,
) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    let export_dir = state.db().path().parent().map_or_else(
        || Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );

    export_report_csv_to_dir(&connection, &export_dir, &request).map_err(Into::into)
}

pub fn query_daily_turnover(
    connection: &Connection,
    query: &ReportDateQuery,
) -> Result<DailyTurnoverReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    substr(s.created_at, 1, 10) AS day,
    SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,
    (
        SELECT COALESCE(SUM(
            CASE WHEN ps.status = 'completed' THEN sp.amount_minor ELSE -sp.amount_minor END
        ), 0)
        FROM sales ps
        JOIN sale_payments sp ON sp.sale_id = ps.id
        WHERE substr(ps.created_at, 1, 10) = substr(s.created_at, 1, 10)
          AND substr(ps.created_at, 1, 10) BETWEEN ?1 AND ?2
          AND sp.payment_method = 'cash'
    ) AS cash_minor,
    (
        SELECT COALESCE(SUM(
            CASE WHEN ps.status = 'completed' THEN sp.amount_minor ELSE -sp.amount_minor END
        ), 0)
        FROM sales ps
        JOIN sale_payments sp ON sp.sale_id = ps.id
        WHERE substr(ps.created_at, 1, 10) = substr(s.created_at, 1, 10)
          AND substr(ps.created_at, 1, 10) BETWEEN ?1 AND ?2
          AND sp.payment_method = 'card'
    ) AS card_minor,
    SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor,
    -SUM(CASE WHEN s.status <> 'completed' THEN s.total_minor ELSE 0 END) AS refunds_or_voids_minor,
    SUM(CASE WHEN s.status <> 'completed' THEN 1 ELSE 0 END) AS refunds_or_voids_count
FROM sales s
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
GROUP BY day
ORDER BY day
"#,
    )?;

    let rows = statement
        .query_map(params![query.from, query.to], |row| {
            Ok(DailyTurnoverRow {
                day: row.get(0)?,
                receipt_count: row.get(1)?,
                cash_minor: row.get(2)?,
                card_minor: row.get(3)?,
                total_minor: row.get(4)?,
                refunds_or_voids_minor: row.get(5)?,
                refunds_or_voids_count: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let total_minor = rows.iter().map(|row| row.total_minor).sum();
    let cash_minor = rows.iter().map(|row| row.cash_minor).sum();
    let card_minor = rows.iter().map(|row| row.card_minor).sum();
    let receipt_count = rows.iter().map(|row| row.receipt_count).sum();
    let average_receipt_minor = if receipt_count == 0 {
        0
    } else {
        total_minor / receipt_count
    };

    Ok(DailyTurnoverReport {
        summary: DailyTurnoverSummary {
            total_minor,
            cash_minor,
            card_minor,
            receipt_count,
            average_receipt_minor,
        },
        rows,
    })
}

pub fn query_shift_turnover(
    connection: &Connection,
    query: &ReportDateQuery,
) -> Result<ShiftTurnoverReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    sh.id,
    sh.opened_at,
    sh.closed_at,
    u.display_name,
    SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,
    COALESCE(SUM(CASE
        WHEN sp.payment_method = 'cash' AND s.status = 'completed' THEN sp.amount_minor
        WHEN sp.payment_method = 'cash' THEN -sp.amount_minor
        ELSE 0
    END), 0) AS cash_minor,
    COALESCE(SUM(CASE
        WHEN sp.payment_method = 'card' AND s.status = 'completed' THEN sp.amount_minor
        WHEN sp.payment_method = 'card' THEN -sp.amount_minor
        ELSE 0
    END), 0) AS card_minor,
    SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor
FROM shifts sh
JOIN users u ON u.id = sh.user_id
JOIN sales s ON s.shift_id = sh.id
LEFT JOIN sale_payments sp ON sp.sale_id = s.id
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
  AND (?3 IS NULL OR sh.id = ?3)
  AND (?4 IS NULL OR sh.user_id = ?4)
GROUP BY sh.id, sh.opened_at, sh.closed_at, u.display_name
ORDER BY sh.opened_at DESC
"#,
    )?;

    let rows = statement
        .query_map(
            params![query.from, query.to, query.shift_id, query.cashier_id],
            |row| {
                Ok(ShiftTurnoverRow {
                    shift_id: row.get(0)?,
                    opened_at: row.get(1)?,
                    closed_at: row.get(2)?,
                    cashier_name: row.get(3)?,
                    receipt_count: row.get(4)?,
                    cash_minor: row.get(5)?,
                    card_minor: row.get(6)?,
                    total_minor: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ShiftTurnoverReport { rows })
}

pub fn query_cashier_turnover(
    connection: &Connection,
    query: &ReportDateQuery,
) -> Result<CashierTurnoverReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    u.id,
    u.display_name,
    SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,
    SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor
FROM users u
JOIN sales s ON s.cashier_id = u.id
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
  AND (?3 IS NULL OR s.shift_id = ?3)
  AND (?4 IS NULL OR u.id = ?4)
GROUP BY u.id, u.display_name
ORDER BY total_minor DESC, u.display_name
"#,
    )?;

    let rows = statement
        .query_map(
            params![query.from, query.to, query.shift_id, query.cashier_id],
            |row| {
                Ok(CashierTurnoverRow {
                    cashier_id: row.get(0)?,
                    cashier_name: row.get(1)?,
                    receipt_count: row.get(2)?,
                    total_minor: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(CashierTurnoverReport { rows })
}

pub fn query_payment_methods(
    connection: &Connection,
    query: &ReportDateQuery,
) -> Result<PaymentMethodReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    sp.payment_method,
    COUNT(DISTINCT CASE WHEN s.status = 'completed' THEN s.id END) AS receipt_count,
    SUM(CASE WHEN s.status = 'completed' THEN sp.amount_minor ELSE -sp.amount_minor END) AS total_minor
FROM sale_payments sp
JOIN sales s ON s.id = sp.sale_id
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
GROUP BY sp.payment_method
ORDER BY CASE sp.payment_method WHEN 'cash' THEN 0 WHEN 'card' THEN 1 ELSE 2 END
"#,
    )?;

    let rows = statement
        .query_map(params![query.from, query.to], |row| {
            Ok(PaymentMethodRow {
                payment_method: row.get(0)?,
                receipt_count: row.get(1)?,
                total_minor: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(PaymentMethodReport { rows })
}

pub fn query_product_sales(
    connection: &Connection,
    query: &ProductSalesQuery,
) -> Result<ProductSalesReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    si.product_id,
    si.product_name,
    si.product_sku,
    SUM(CASE WHEN s.status = 'completed' THEN ABS(si.quantity_milli) ELSE -ABS(si.quantity_milli) END) AS quantity_milli,
    SUM(CASE WHEN s.status = 'completed' THEN si.total_minor ELSE -si.total_minor END) AS revenue_minor,
    SUM(CASE WHEN s.status = 'completed' THEN si.discount_minor ELSE -si.discount_minor END) AS discount_minor,
    SUM(
        CASE WHEN s.status = 'completed' THEN si.total_minor ELSE -si.total_minor END
        -
        CASE WHEN s.status = 'completed'
            THEN COALESCE(p.purchase_price_minor, 0) * ABS(si.quantity_milli) / 1000
            ELSE -(COALESCE(p.purchase_price_minor, 0) * ABS(si.quantity_milli) / 1000)
        END
    ) AS estimated_margin_minor
FROM sale_items si
JOIN sales s ON s.id = si.sale_id
LEFT JOIN products p ON p.id = si.product_id
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
  AND (?3 IS NULL OR p.category_id = ?3)
  AND (?4 IS NULL OR si.product_id = ?4)
GROUP BY si.product_id, si.product_name, si.product_sku
ORDER BY revenue_minor DESC, si.product_name
"#,
    )?;

    let rows = statement
        .query_map(
            params![query.from, query.to, query.category_id, query.product_id],
            |row| {
                Ok(ProductSalesRow {
                    product_id: row.get(0)?,
                    product_name: row.get(1)?,
                    product_sku: row.get(2)?,
                    quantity_milli: row.get(3)?,
                    revenue_minor: row.get(4)?,
                    discount_minor: row.get(5)?,
                    estimated_margin_minor: row.get(6)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ProductSalesReport { rows })
}

pub fn query_category_sales(
    connection: &Connection,
    query: &ReportDateQuery,
) -> Result<CategorySalesReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    c.id,
    COALESCE(c.name, 'Bez kategorije') AS category_name,
    SUM(CASE WHEN s.status = 'completed' THEN ABS(si.quantity_milli) ELSE -ABS(si.quantity_milli) END) AS quantity_milli,
    SUM(CASE WHEN s.status = 'completed' THEN si.total_minor ELSE -si.total_minor END) AS revenue_minor,
    SUM(CASE WHEN s.status = 'completed' THEN si.discount_minor ELSE -si.discount_minor END) AS discount_minor,
    SUM(
        CASE WHEN s.status = 'completed' THEN si.total_minor ELSE -si.total_minor END
        -
        CASE WHEN s.status = 'completed'
            THEN COALESCE(p.purchase_price_minor, 0) * ABS(si.quantity_milli) / 1000
            ELSE -(COALESCE(p.purchase_price_minor, 0) * ABS(si.quantity_milli) / 1000)
        END
    ) AS estimated_margin_minor
FROM sale_items si
JOIN sales s ON s.id = si.sale_id
LEFT JOIN products p ON p.id = si.product_id
LEFT JOIN categories c ON c.id = p.category_id
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
GROUP BY c.id, category_name
ORDER BY revenue_minor DESC, category_name
"#,
    )?;

    let rows = statement
        .query_map(params![query.from, query.to], |row| {
            Ok(CategorySalesRow {
                category_id: row.get(0)?,
                category_name: row.get(1)?,
                quantity_milli: row.get(2)?,
                revenue_minor: row.get(3)?,
                discount_minor: row.get(4)?,
                estimated_margin_minor: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(CategorySalesReport { rows })
}

pub fn query_low_stock(connection: &Connection) -> Result<LowStockReport, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    p.id,
    p.name,
    p.sku,
    COALESCE(ib.quantity_milli, 0) AS current_stock_milli,
    p.minimum_stock_milli,
    COALESCE(ib.quantity_milli, 0) - p.minimum_stock_milli AS difference_milli,
    (
        SELECT MAX(im.created_at)
        FROM inventory_movements im
        WHERE im.product_id = p.id
    ) AS last_movement_at
FROM products p
LEFT JOIN inventory_balances ib ON ib.product_id = p.id
WHERE p.active = 1
  AND COALESCE(ib.quantity_milli, 0) < p.minimum_stock_milli
ORDER BY difference_milli ASC, p.name
"#,
    )?;

    let rows = statement
        .query_map([], |row| {
            Ok(LowStockRow {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                product_sku: row.get(2)?,
                current_stock_milli: row.get(3)?,
                minimum_stock_milli: row.get(4)?,
                difference_milli: row.get(5)?,
                last_movement_at: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(LowStockReport { rows })
}

pub fn query_list_shifts(connection: &Connection) -> Result<Vec<ShiftListItem>, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT
    sh.id,
    sh.opened_at,
    sh.closed_at,
    u.display_name
FROM shifts sh
JOIN users u ON u.id = sh.user_id
ORDER BY sh.opened_at DESC
LIMIT 200
"#,
    )?;

    let rows = statement
        .query_map([], |row| {
            Ok(ShiftListItem {
                id: row.get(0)?,
                opened_at: row.get(1)?,
                closed_at: row.get(2)?,
                cashier_name: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

pub fn export_report_csv_to_dir(
    connection: &Connection,
    export_dir: &Path,
    request: &ExportReportRequest,
) -> Result<ExportedFile, AppError> {
    fs::create_dir_all(export_dir)?;

    let (csv, row_count) = build_report_csv(connection, request)?;
    let file_name = format!(
        "{}-{}-{}.csv",
        request.report_type.file_slug(),
        request.query.from,
        request.query.to
    );
    let path = export_dir.join(&file_name);
    fs::write(&path, csv)?;

    Ok(ExportedFile {
        file_name,
        path: path.display().to_string(),
        mime_type: "text/csv",
        row_count,
    })
}

fn build_report_csv(
    connection: &Connection,
    request: &ExportReportRequest,
) -> Result<(String, usize), AppError> {
    match request.report_type {
        ExportReportType::DailyTurnover => {
            let report = query_daily_turnover(connection, &request.query.date_query())?;
            let mut lines = vec![csv_line(&[
                "Dan",
                "Broj racuna",
                "Gotovina",
                "Kartica",
                "Ukupno",
                "Povrati i storniranja",
            ])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    row.day.as_str(),
                    &row.receipt_count.to_string(),
                    &row.cash_minor.to_string(),
                    &row.card_minor.to_string(),
                    &row.total_minor.to_string(),
                    &row.refunds_or_voids_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::ShiftTurnover => {
            let report = query_shift_turnover(connection, &request.query.date_query())?;
            let mut lines = vec![csv_line(&[
                "Smena",
                "Otvorena",
                "Kasir",
                "Broj racuna",
                "Gotovina",
                "Kartica",
                "Ukupno",
            ])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    &row.shift_id.to_string(),
                    row.opened_at.as_str(),
                    row.cashier_name.as_str(),
                    &row.receipt_count.to_string(),
                    &row.cash_minor.to_string(),
                    &row.card_minor.to_string(),
                    &row.total_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::CashierTurnover => {
            let report = query_cashier_turnover(connection, &request.query.date_query())?;
            let mut lines = vec![csv_line(&["Kasir", "Broj racuna", "Ukupno"])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    row.cashier_name.as_str(),
                    &row.receipt_count.to_string(),
                    &row.total_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::PaymentMethods => {
            let report = query_payment_methods(connection, &request.query.date_query())?;
            let mut lines = vec![csv_line(&["Nacin placanja", "Broj racuna", "Ukupno"])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    payment_method_label(row.payment_method.as_str()),
                    &row.receipt_count.to_string(),
                    &row.total_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::ProductSales => {
            let report = query_product_sales(connection, &request.query)?;
            let mut lines = vec![csv_line(&[
                "Artikal",
                "SKU",
                "Kolicina",
                "Promet",
                "Popust",
                "Procena marze",
            ])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    row.product_name.as_str(),
                    row.product_sku.as_str(),
                    &row.quantity_milli.to_string(),
                    &row.revenue_minor.to_string(),
                    &row.discount_minor.to_string(),
                    &row.estimated_margin_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::CategorySales => {
            let report = query_category_sales(connection, &request.query.date_query())?;
            let mut lines = vec![csv_line(&[
                "Kategorija",
                "Kolicina",
                "Promet",
                "Popust",
                "Procena marze",
            ])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    row.category_name.as_str(),
                    &row.quantity_milli.to_string(),
                    &row.revenue_minor.to_string(),
                    &row.discount_minor.to_string(),
                    &row.estimated_margin_minor.to_string(),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
        ExportReportType::LowStock => {
            let report = query_low_stock(connection)?;
            let mut lines = vec![csv_line(&[
                "Artikal",
                "SKU",
                "Trenutno",
                "Minimum",
                "Razlika",
                "Poslednja promena",
            ])];

            for row in &report.rows {
                lines.push(csv_line(&[
                    row.product_name.as_str(),
                    row.product_sku.as_str(),
                    &row.current_stock_milli.to_string(),
                    &row.minimum_stock_milli.to_string(),
                    &row.difference_milli.to_string(),
                    row.last_movement_at.as_deref().unwrap_or(""),
                ]));
            }

            Ok((lines.join("\n"), report.rows.len()))
        }
    }
}

fn csv_line(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| csv_escape(field))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn payment_method_label(payment_method: &str) -> &str {
    match payment_method {
        "cash" => "Gotovina",
        "card" => "Kartica",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::{params, Connection};
    use tauri::Manager;

    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

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

    fn with_seeded_reports_database(test_name: &str, test: impl FnOnce(&Connection)) {
        let db_path = test_database_path(test_name);

        {
            let db = Db::new(&db_path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            seed_reports_data(&connection);
            test(&connection);
        }

        fs::remove_file(&db_path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                db_path.display()
            )
        });
    }

    fn seed_reports_data(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO users (id, username, display_name, role, created_at, updated_at)
                 VALUES (2, 'mira', 'Mira Kasir', 'cashier', '2026-06-17T07:00:00Z', '2026-06-17T07:00:00Z')",
                [],
            )
            .expect("cashier should insert");

        connection
            .execute(
                "INSERT INTO shifts (
                    id,
                    user_id,
                    opened_at,
                    opening_cash_minor,
                    expected_cash_minor,
                    status,
                    created_at,
                    updated_at
                 )
                 VALUES (1, 2, '2026-06-17T07:30:00Z', 0, 0, 'open', '2026-06-17T07:30:00Z', '2026-06-17T07:30:00Z')",
                [],
            )
            .expect("shift should insert");

        connection
            .execute(
                "INSERT INTO categories (id, name, created_at, updated_at)
                 VALUES (1, 'Pica', '2026-06-17T07:00:00Z', '2026-06-17T07:00:00Z')",
                [],
            )
            .expect("category should insert");

        connection
            .execute(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-06-17T07:00:00Z', '2026-06-17T07:00:00Z')",
                [],
            )
            .expect("tax rate should insert");

        for (id, name, sku, sale_price, purchase_price, minimum, balance) in [
            (1, "Kafa 200g", "KAF-200", 700, 420, 5_000, 3_000),
            (2, "Sok 1l", "SOK-1L", 300, 170, 1_000, 1_000),
        ] {
            connection
                .execute(
                    "INSERT INTO products (
                        id,
                        name,
                        sku,
                        category_id,
                        sale_price_minor,
                        purchase_price_minor,
                        tax_rate_id,
                        minimum_stock_milli,
                        created_at,
                        updated_at
                     )
                     VALUES (?1, ?2, ?3, 1, ?4, ?5, 1, ?6, '2026-06-17T07:00:00Z', '2026-06-17T07:00:00Z')",
                    params![id, name, sku, sale_price, purchase_price, minimum],
                )
                .expect("product should insert");

            connection
                .execute(
                    "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                     VALUES (?1, ?2, '2026-06-17T07:30:00Z')",
                    params![id, balance],
                )
                .expect("inventory balance should insert");
        }

        connection
            .execute(
                "INSERT INTO inventory_movements (
                    product_id,
                    movement_type,
                    quantity_milli,
                    reason,
                    created_at
                 )
                 VALUES (1, 'sale', -2000, 'Prodaja', '2026-06-17T12:00:00Z')",
                [],
            )
            .expect("inventory movement should insert");

        for (sale_id, receipt, status, subtotal, discount, tax, total, created_at, cash, card) in [
            (
                1,
                "R-001",
                "completed",
                10_500,
                500,
                1_667,
                10_000,
                "2026-06-17T09:00:00Z",
                6_000,
                4_000,
            ),
            (
                2,
                "R-002",
                "completed",
                5_000,
                0,
                833,
                5_000,
                "2026-06-17T10:00:00Z",
                5_000,
                0,
            ),
            (
                3,
                "R-003",
                "voided",
                3_000,
                0,
                500,
                3_000,
                "2026-06-17T11:00:00Z",
                3_000,
                0,
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO sales (
                        id,
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
                     VALUES (?1, ?2, 1, 2, ?3, 'not_fiscalized', ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![sale_id, receipt, status, subtotal, discount, tax, total, created_at],
                )
                .expect("sale should insert");

            if cash > 0 {
                connection
                    .execute(
                        "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                         VALUES (?1, 'cash', ?2, ?3)",
                        params![sale_id, cash, created_at],
                    )
                    .expect("cash payment should insert");
            }

            if card > 0 {
                connection
                    .execute(
                        "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                         VALUES (?1, 'card', ?2, ?3)",
                        params![sale_id, card, created_at],
                    )
                    .expect("card payment should insert");
            }
        }

        for (sale_id, product_id, name, sku, quantity, unit_price, discount, tax, total) in [
            (
                1,
                1,
                "Kafa 200g",
                "KAF-200",
                1_000,
                7_000,
                200,
                1_133,
                7_000,
            ),
            (1, 2, "Sok 1l", "SOK-1L", 2_000, 1_500, 300, 534, 3_000),
            (2, 1, "Kafa 200g", "KAF-200", 1_000, 5_000, 0, 833, 5_000),
            (3, 2, "Sok 1l", "SOK-1L", 1_000, 3_000, 0, 500, 3_000),
        ] {
            connection
                .execute(
                    "INSERT INTO sale_items (
                        sale_id,
                        product_id,
                        product_name,
                        product_sku,
                        quantity_milli,
                        unit_price_minor,
                        discount_minor,
                        tax_rate_basis_points,
                        tax_minor,
                        total_minor
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 2000, ?8, ?9)",
                    params![
                        sale_id, product_id, name, sku, quantity, unit_price, discount, tax, total
                    ],
                )
                .expect("sale item should insert");
        }
    }

    #[test]
    fn query_daily_turnover_groups_completed_sales_and_reduces_voids() {
        with_seeded_reports_database(
            "query_daily_turnover_groups_completed_sales_and_reduces_voids",
            |connection| {
                let report = super::query_daily_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: None,
                    },
                )
                .expect("daily turnover should query");

                assert_eq!(report.rows.len(), 1);
                assert_eq!(report.summary.total_minor, 12_000);
                assert_eq!(report.summary.cash_minor, 8_000);
                assert_eq!(report.summary.card_minor, 4_000);
                assert_eq!(report.summary.receipt_count, 2);
                assert_eq!(report.rows[0].refunds_or_voids_minor, -3_000);
            },
        );
    }

    #[test]
    fn query_payment_methods_handles_mixed_payments_and_voids() {
        with_seeded_reports_database(
            "query_payment_methods_handles_mixed_payments_and_voids",
            |connection| {
                let report = super::query_payment_methods(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: None,
                    },
                )
                .expect("payment methods should query");

                assert_eq!(report.rows.len(), 2);
                assert_eq!(report.rows[0].payment_method, "cash");
                assert_eq!(report.rows[0].total_minor, 8_000);
                assert_eq!(report.rows[1].payment_method, "card");
                assert_eq!(report.rows[1].total_minor, 4_000);
            },
        );
    }

    #[test]
    fn query_product_sales_returns_net_quantity_revenue_and_margin_estimate() {
        with_seeded_reports_database(
            "query_product_sales_returns_net_quantity_revenue_and_margin_estimate",
            |connection| {
                let report = super::query_product_sales(
                    connection,
                    &super::ProductSalesQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        category_id: None,
                        product_id: None,
                    },
                )
                .expect("product sales should query");

                assert_eq!(report.rows.len(), 2);
                assert_eq!(report.rows[0].product_name, "Kafa 200g");
                assert_eq!(report.rows[0].quantity_milli, 2_000);
                assert_eq!(report.rows[0].revenue_minor, 12_000);
                assert_eq!(report.rows[0].estimated_margin_minor, 11_160);
                assert_eq!(report.rows[1].product_name, "Sok 1l");
                assert_eq!(report.rows[1].quantity_milli, 1_000);
                assert_eq!(report.rows[1].revenue_minor, 0);
            },
        );
    }

    #[test]
    fn query_low_stock_returns_active_products_below_minimum_with_last_movement() {
        with_seeded_reports_database(
            "query_low_stock_returns_active_products_below_minimum_with_last_movement",
            |connection| {
                let report = super::query_low_stock(connection).expect("low stock should query");

                assert_eq!(report.rows.len(), 1);
                assert_eq!(report.rows[0].product_name, "Kafa 200g");
                assert_eq!(report.rows[0].current_stock_milli, 3_000);
                assert_eq!(report.rows[0].minimum_stock_milli, 5_000);
                assert_eq!(report.rows[0].difference_milli, -2_000);
                assert_eq!(
                    report.rows[0].last_movement_at.as_deref(),
                    Some("2026-06-17T12:00:00Z")
                );
            },
        );
    }

    #[test]
    fn query_shift_turnover_filters_by_shift_id() {
        with_seeded_reports_database("query_shift_turnover_filters_by_shift_id", |connection| {
            let matched = super::query_shift_turnover(
                connection,
                &super::ReportDateQuery {
                    from: "2026-06-17".to_string(),
                    to: "2026-06-17".to_string(),
                    shift_id: Some(1),
                    cashier_id: None,
                },
            )
            .expect("shift turnover should query");
            assert_eq!(matched.rows.len(), 1);
            assert_eq!(matched.rows[0].shift_id, 1);

            let unmatched = super::query_shift_turnover(
                connection,
                &super::ReportDateQuery {
                    from: "2026-06-17".to_string(),
                    to: "2026-06-17".to_string(),
                    shift_id: Some(999),
                    cashier_id: None,
                },
            )
            .expect("shift turnover should query");
            assert!(unmatched.rows.is_empty());
        });
    }

    #[test]
    fn query_list_shifts_returns_seeded_shift_with_cashier_name() {
        with_seeded_reports_database(
            "query_list_shifts_returns_seeded_shift_with_cashier_name",
            |connection| {
                let shifts = super::query_list_shifts(connection).expect("shift list should query");

                assert_eq!(shifts.len(), 1);
                assert_eq!(shifts[0].id, 1);
                assert_eq!(shifts[0].cashier_name, "Mira Kasir");
                assert_eq!(shifts[0].opened_at, "2026-06-17T07:30:00Z");
                assert_eq!(shifts[0].closed_at, None);
            },
        );
    }

    #[test]
    fn query_cashier_turnover_filters_by_cashier_id() {
        with_seeded_reports_database(
            "query_cashier_turnover_filters_by_cashier_id",
            |connection| {
                let matched = super::query_cashier_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: Some(2),
                    },
                )
                .expect("cashier turnover should query");
                assert_eq!(matched.rows.len(), 1);
                assert_eq!(matched.rows[0].cashier_id, 2);

                let unmatched = super::query_cashier_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: Some(999),
                    },
                )
                .expect("cashier turnover should query");
                assert!(unmatched.rows.is_empty());
            },
        );
    }

    #[test]
    fn export_report_csv_writes_daily_turnover_with_serbian_headers() {
        with_seeded_reports_database(
            "export_report_csv_writes_daily_turnover_with_serbian_headers",
            |connection| {
                let export_dir =
                    std::env::temp_dir().join("vantumpos-report-export-test-daily-turnover");
                let _ = fs::remove_dir_all(&export_dir);

                let exported = super::export_report_csv_to_dir(
                    connection,
                    &export_dir,
                    &super::ExportReportRequest {
                        report_type: super::ExportReportType::DailyTurnover,
                        query: super::ProductSalesQuery {
                            from: "2026-06-17".to_string(),
                            to: "2026-06-17".to_string(),
                            category_id: None,
                            product_id: None,
                        },
                    },
                )
                .expect("daily turnover csv should export");

                let csv = fs::read_to_string(&exported.path).expect("csv should be readable");
                assert!(csv
                    .starts_with("Dan,Broj racuna,Gotovina,Kartica,Ukupno,Povrati i storniranja"));
                assert!(csv.contains("2026-06-17,2,8000,4000,12000,-3000"));
                assert_eq!(exported.row_count, 1);

                fs::remove_dir_all(export_dir).expect("export dir should be removed");
            },
        );
    }

    #[test]
    fn reports_command_rejected_for_cashier() {
        let db_path = test_database_path("reports_command_rejected_for_cashier");

        {
            let db = Db::new(&db_path).expect("database should initialize");
            let state = AppState::new(db);
            sign_in_cashier(&state);

            // Reports is an admin-only surface, so the command wrapper must reject a
            // cashier before any query runs. The command takes a Tauri `State`, so we
            // build a headless mock app to obtain a real managed state.
            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");
            let managed = app.state::<AppState>();

            let error = super::reports_daily_turnover(
                managed,
                super::ReportDateQuery {
                    from: "2026-06-17".to_string(),
                    to: "2026-06-17".to_string(),
                    shift_id: None,
                    cashier_id: None,
                },
            )
            .expect_err("cashier should not read reports");

            assert_eq!(error.code, "forbidden");
        }

        fs::remove_file(&db_path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                db_path.display()
            )
        });
    }
}
