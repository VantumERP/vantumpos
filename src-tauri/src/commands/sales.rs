use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::inventory::{write_stock_movement, StockMovementWrite};
use crate::commands::settings::{
    ReceiptSettings, SalesSettings, ShopProfile, EUR_RATE_KEY, RECEIPT_SETTINGS_KEY,
    SALES_SETTINGS_KEY, SHOP_PROFILE_KEY,
};
use crate::db::Db;
use crate::nbs_rate::EurRate;
use crate::state::AppState;

/// The share of the čl. 46 st. 1 cap at which the till starts warning. It is a
/// [PRUDENTIAL] early warning, not a statutory line — nothing below the cap is
/// unlawful.
const AML_SOFT_RATIO_PERCENT: i64 = 80;

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
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Card,
    /// The lawful alternative čl. 46 st. 1 points the customer to when the cash
    /// cap is reached: the money settles in the bank, never in the drawer.
    BankTransfer,
}

impl PaymentMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Card => "card",
            Self::BankTransfer => "bank_transfer",
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
    /// What the operator typed when acknowledging the AML warning. Recorded,
    /// never required: the sale is never rejected on an AML result.
    #[serde(default)]
    pub aml_ack_reason: Option<String>,
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

/// One article rung above the price its outlet published (req. 12).
///
/// ZZP čl. 6 st. 4 binds a trader **who publishes** a cenovnik to adhere to the
/// prices in it, so this is the till's cheapest way of keeping the shop out of
/// čl. 207/206 territory. It is a warning and only a warning: the register has to
/// be able to record what actually happened at the counter, and a hard block over
/// a stale snapshot would be worse than the exposure it prevents.
///
/// Below-published is silent — a discount is not a breach — which is also what
/// keeps the guard to a single comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceDivergence {
    pub product_id: i64,
    pub product_name: String,
    pub product_sku: String,
    /// What the till is about to charge for one unit, in para.
    pub charged_unit_price_minor: i64,
    /// What the outlet's current cenovnik says for that article, in para.
    pub published_unit_price_minor: i64,
    /// The snapshot the comparison was made against. An outlet's archive holds
    /// many files and čl. 6 st. 4 binds the shop only to the one in force, so a
    /// divergence that did not name its exhibit would be an accusation with none.
    pub snapshot_id: i64,
    pub snapshot_generated_at: String,
    pub snapshot_content_hash: String,
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
    stored_bank_transfer_minor: i64,
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
    // The acting user is read from the server-side session, never from the
    // request payload: it is what the AML audit entry attributes the accepted
    // cash to. A till with no session still sells — the entry then falls back
    // to the cashier the open shift belongs to.
    let acting_user_id = state.session_user_id().map_err(CommandError::from)?;

    complete_sale_transaction(state.db(), request, acting_user_id).map_err(Into::into)
}

/// Asked by the till before the money changes hands. `cash_minor` is the cash
/// line of the tender, never the invoice total. The verdict is advisory — the
/// caller decides how to warn, and `sales_complete` accepts the sale either
/// way.
#[tauri::command]
pub fn sales_assess_cash_payment(
    state: State<'_, AppState>,
    cash_minor: i64,
) -> Result<crate::aml::AmlAssessment, CommandError> {
    let profile = crate::commands::settings::load_shop_profile(state.inner())?;
    let rate = crate::commands::settings::load_eur_rate(state.inner())?;
    Ok(crate::aml::assess_cash_payment(
        cash_minor,
        rate.as_ref(),
        &profile,
        AML_SOFT_RATIO_PERCENT,
    ))
}

/// Asked by the till while the cart is being built, so the operator learns about
/// a divergence before the money changes hands rather than from a log afterwards.
///
/// Advisory in exactly the way `sales_assess_cash_payment` is: `sales_complete`
/// accepts the sale whatever this returns, and an empty answer is the ordinary
/// case. The caller decides how loudly to say it (req. 12 — „refuse or loudly
/// warn“; this product warns, see [`PriceDivergence`]).
#[tauri::command]
pub fn sales_assess_price_integrity(
    state: State<'_, AppState>,
    request: SaleDraftRequest,
) -> Result<Vec<PriceDivergence>, CommandError> {
    assess_draft_price_integrity(state.db(), request).map_err(Into::into)
}

pub fn build_sale_preview(db: &Db, request: SaleDraftRequest) -> Result<SalePreview, AppError> {
    let connection = db.open()?;
    let computation = compute_sale(&connection, &request)?;

    Ok(computation.preview)
}

/// The draft priced the way the till would ring it, measured against the outlet's
/// published cenovnik.
pub fn assess_draft_price_integrity(
    db: &Db,
    request: SaleDraftRequest,
) -> Result<Vec<PriceDivergence>, AppError> {
    let connection = db.open()?;
    let computation = compute_sale(&connection, &request)?;

    Ok(assess_price_integrity(&connection, &computation.lines))
}

pub fn complete_sale_transaction(
    db: &Db,
    request: CompleteSaleRequest,
    acting_user_id: Option<i64>,
) -> Result<CompletedSale, AppError> {
    let mut connection = db.open()?;
    let tx = connection.transaction()?;
    let CompleteSaleRequest {
        items,
        receipt_discount,
        payments,
        allow_stock_override,
        aml_ack_reason,
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
    // čl. 46 st. 1 keys to the cash actually received, so the assessment reads
    // the cash line — not the invoice total, and not the tendered amount, which
    // still carries the change. A verdict never rejects the sale.
    let aml = assess_sale_cash(&tx, payment.stored_cash_minor, aml_ack_reason)?;

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
            aml_cash_minor,
            aml_rate_minor,
            aml_rate_date,
            aml_rate_source,
            aml_ack_reason,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, 'completed', 'not_fiscalized', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            local_receipt_number,
            shift.id,
            shift.cashier_id,
            computation.preview.subtotal_minor,
            computation.preview.discount_minor,
            computation.preview.tax_minor,
            computation.preview.total_minor,
            aml.cash_minor,
            aml.rate_minor,
            aml.rate_date,
            aml.rate_source,
            aml.ack_reason,
            created_at
        ],
    )?;
    let sale_id = tx.last_insert_rowid();

    // §3 req 8 [PRUDENTIAL]: a breach — not the soft warning — also leaves an
    // immutable audit entry. It is written here, on the sale's own transaction
    // and before the rest of the sale, so the two can only ever exist together.
    if let Some(threshold_minor) = aml.breached_threshold_minor {
        insert_aml_breach_event(
            &tx,
            AmlBreachEvent {
                sale_id,
                local_receipt_number: &local_receipt_number,
                threshold_minor,
                provenance: &aml,
                // The signed-in operator when there is one; otherwise the
                // cashier the open shift belongs to, who is by construction the
                // person at the till. An audit entry naming nobody is a weaker
                // record than one naming the shift's owner.
                user_id: acting_user_id.unwrap_or(shift.cashier_id),
                created_at: &created_at,
            },
        )?;
    }

    // Req. 12 / čl. 6 st. 4: an article charged above the price its outlet
    // published leaves an entry in the same never-deleted trail. The sale is
    // never refused for it — the guard warned while the cart was being built
    // (`sales_assess_price_integrity`), and the register has to be able to record
    // what actually happened at the counter.
    for divergence in assess_price_integrity(&tx, &computation.lines) {
        insert_price_divergence_event(
            &tx,
            &divergence,
            sale_id,
            &local_receipt_number,
            // The signed-in operator when there is one, otherwise the cashier the
            // open shift belongs to — the same reading the AML entry takes.
            acting_user_id.unwrap_or(shift.cashier_id),
            &created_at,
        )?;
    }

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
    insert_payment_if_present(
        &tx,
        sale_id,
        PaymentMethod::BankTransfer,
        payment.stored_bank_transfer_minor,
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
                "Količina mora biti veća od nule.",
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
        .ok_or_else(|| AppError::not_found("Artikal nije pronađen."))
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

/// What gets written into the five AML columns of the sale row. Every field is
/// `None` unless the assessment actually fired: an ordinary sale must leave no
/// trace, so the columns themselves are the record that a warning was shown.
#[derive(Debug, Default)]
struct AmlProvenance {
    cash_minor: Option<i64>,
    rate_minor: Option<i64>,
    rate_date: Option<String>,
    rate_source: Option<String>,
    ack_reason: Option<String>,
    /// The cap the cash line was measured against, set **only** when it was
    /// reached or passed. A `near_threshold` result fills the columns above but
    /// leaves this `None`: the soft line is a [PRUDENTIAL] early warning, and
    /// nothing below the cap is unlawful, so it earns no compliance event.
    breached_threshold_minor: Option<i64>,
}

/// The AML inputs are read on the sale's own connection: this runs inside the
/// sale transaction, and `complete_sale_transaction` holds a `Db` rather than
/// the `AppState` the settings loaders take.
///
/// A stored value that will not deserialize degrades to `default_value` instead
/// of erroring. An AML verdict must never reject a sale, so a corrupt
/// `shop_profile` or `eur_rate` row has to land in the `rate_unavailable` path
/// rather than turn the register into a till that cannot sell anything. Genuine
/// SQL errors still propagate.
fn read_json_setting<T>(connection: &Connection, key: &str, default_value: T) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned,
{
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;

    Ok(stored
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or(default_value))
}

/// Assesses the cash line and reduces the verdict to what the sale row keeps.
///
/// Without a rate nothing fires — `assess_cash_payment` reports
/// `rate_unavailable` rather than guessing — so the columns stay `NULL` and the
/// sale still completes.
fn assess_sale_cash(
    connection: &Connection,
    cash_minor: i64,
    ack_reason: Option<String>,
) -> Result<AmlProvenance, AppError> {
    let profile: ShopProfile =
        read_json_setting(connection, SHOP_PROFILE_KEY, ShopProfile::default())?;
    let rate: Option<EurRate> = read_json_setting(connection, EUR_RATE_KEY, None)?;
    let assessment = crate::aml::assess_cash_payment(
        cash_minor,
        rate.as_ref(),
        &profile,
        AML_SOFT_RATIO_PERCENT,
    );

    match assessment.rate {
        Some(rate) if assessment.breached || assessment.near_threshold => Ok(AmlProvenance {
            cash_minor: Some(assessment.cash_minor),
            rate_minor: Some(rate.rate_minor),
            rate_date: Some(rate.rate_date),
            rate_source: Some(rate.source.as_str().to_string()),
            ack_reason: ack_reason
                .map(|reason| reason.trim().to_string())
                .filter(|reason| !reason.is_empty()),
            breached_threshold_minor: assessment.breached.then_some(assessment.threshold_minor),
        }),
        _ => Ok(AmlProvenance::default()),
    }
}

/// Compares each rung unit price against the outlet's current published
/// cenovnik, and reports only the lines above it (req. 12).
///
/// **Infallible on purpose.** An archive this cannot read degrades to „no
/// guard“ — the same answer as an outlet that has published nothing — and never
/// to an error, because čl. 6 is not a reason a till cannot sell. The failure is
/// logged instead; a shop whose archive is unreadable has a problem the Task 8
/// panel is the place to learn about, not the queue at the counter.
///
/// One entry per article, not per line: the unit price comes from the catalog, so
/// the same article on two lines carries the same figure, and two identical
/// entries would be two accusations over one departure from one published price.
///
/// **The comparison is against the unit price the article was offered at, not
/// against the line total after a discount.** Čl. 6 st. 1 is about the article's
/// prodajna cena; a discount is a reduction granted on that price, not a
/// different price for the article — and a discount that cancelled the warning
/// would be the obvious way to ring above the published cenovnik unremarked.
fn assess_price_integrity(connection: &Connection, lines: &[ComputedLine]) -> Vec<PriceDivergence> {
    let published = match crate::commands::cenovnik::current_published_cenovnik(connection) {
        Ok(Some(published)) => published,
        Ok(None) => return Vec::new(),
        Err(error) => {
            log::warn!("Objavljeni cenovnik nije pročitan za proveru cena na kasi: {error}");
            return Vec::new();
        }
    };

    let mut divergences: Vec<PriceDivergence> = Vec::new();
    for line in lines {
        let Some(published_unit_price_minor) = published.prodajna_cena(&line.product.sku) else {
            // An article the published file does not carry — created since the
            // last republish — has no published price to depart from, and
            // treating „absent“ as „zero“ would accuse the shop on every sale.
            continue;
        };
        // Strictly greater: charging exactly the published price is adherence,
        // and charging less is a discount.
        if line.unit_price_minor <= published_unit_price_minor {
            continue;
        }
        if divergences
            .iter()
            .any(|divergence| divergence.product_id == line.product.id)
        {
            continue;
        }

        divergences.push(PriceDivergence {
            product_id: line.product.id,
            product_name: line.product.name.clone(),
            product_sku: line.product.sku.clone(),
            charged_unit_price_minor: line.unit_price_minor,
            published_unit_price_minor,
            snapshot_id: published.snapshot_id,
            snapshot_generated_at: published.generated_at.clone(),
            snapshot_content_hash: published.content_hash.clone(),
        });
    }

    divergences
}

/// Appends the divergence entry, on the sale's own transaction.
///
/// The entry and the sale commit together or not at all, exactly as the AML one
/// does: „warn, never block“ is a rule about the **price**, not a licence for a
/// charge above the published cenovnik to exist with no record that it did.
///
/// Stamped with the sale's own `created_at` rather than `datetime('now')` — the
/// two records must not be able to disagree about when the price was charged.
fn insert_price_divergence_event(
    tx: &Connection,
    divergence: &PriceDivergence,
    sale_id: i64,
    local_receipt_number: &str,
    user_id: i64,
    created_at: &str,
) -> Result<(), AppError> {
    let detail = serde_json::json!({
        "sale_id": sale_id,
        "local_receipt_number": local_receipt_number,
        "product_id": divergence.product_id,
        "sifra": divergence.product_sku,
        "naziv": divergence.product_name,
        // Both in para, never a rendered figure: the record is read back by code
        // as often as by a person.
        "naplacena_cena_minor": divergence.charged_unit_price_minor,
        "objavljena_cena_minor": divergence.published_unit_price_minor,
        // Which published file, by id, by date and by bytes.
        "snapshot_id": divergence.snapshot_id,
        "snapshot_generated_at": divergence.snapshot_generated_at,
        "snapshot_content_hash": divergence.snapshot_content_hash,
        "note": "Artikal je naplaćen iznad cene iz objavljenog cenovnika \
                 (čl. 6 st. 4 Zakona o zaštiti potrošača). Prodaja nije zaustavljena.",
    })
    .to_string();

    tx.execute(
        "INSERT INTO compliance_log (event_type, detail_json, user_id, created_at)
         VALUES ('cenovnik_price_divergence', ?1, ?2, ?3)",
        params![detail, user_id, created_at],
    )?;

    Ok(())
}

/// What a breach of the čl. 46 st. 1 cash cap writes into the never-deleted
/// `compliance_log`, in the shape `backup.rs` uses for the other two event
/// types. §3 req 8 [PRUDENTIAL]: the soft block owes an immutable audit entry,
/// so an inspection can reproduce the decision from the log alone.
struct AmlBreachEvent<'a> {
    sale_id: i64,
    local_receipt_number: &'a str,
    threshold_minor: i64,
    provenance: &'a AmlProvenance,
    user_id: i64,
    created_at: &'a str,
}

/// Appends the breach entry. Takes the sale's own transaction: the entry and
/// the sale commit together or not at all, so a breaching sale can never exist
/// without its audit entry, nor the entry without its sale.
///
/// The row is stamped with the sale's `created_at` rather than
/// `datetime('now')` — the two records must not be able to disagree about when
/// the cash was accepted.
fn insert_aml_breach_event(tx: &Connection, event: AmlBreachEvent<'_>) -> Result<(), AppError> {
    let detail = serde_json::json!({
        "sale_id": event.sale_id,
        "local_receipt_number": event.local_receipt_number,
        // The cash the drawer kept, in para — never the invoice total.
        "cash_minor": event.provenance.cash_minor,
        "threshold_minor": event.threshold_minor,
        "rate_minor": event.provenance.rate_minor,
        "rate_date": event.provenance.rate_date,
        "rate_source": event.provenance.rate_source,
        "ack_reason": event.provenance.ack_reason,
        "note": "Gotovina zadržana u iznosu na ili iznad praga iz čl. 46 st. 1 \
                 Zakona o sprečavanju pranja novca i finansiranja terorizma.",
    })
    .to_string();

    tx.execute(
        "INSERT INTO compliance_log (event_type, detail_json, user_id, created_at)
         VALUES ('aml_cash_threshold', ?1, ?2, ?3)",
        params![detail, event.user_id, event.created_at],
    )?;

    Ok(())
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
    let mut bank_transfer_minor = 0_i64;

    for payment in payments {
        if payment.amount_minor < 0 {
            return Err(AppError::business(
                "payment_mismatch",
                "Plaćanja se ne poklapaju sa ukupnim iznosom.",
            ));
        }

        match payment.method {
            PaymentMethod::Cash => {
                cash_received_minor = checked_add(cash_received_minor, payment.amount_minor)?;
            }
            PaymentMethod::Card => {
                card_minor = checked_add(card_minor, payment.amount_minor)?;
            }
            PaymentMethod::BankTransfer => {
                bank_transfer_minor = checked_add(bank_transfer_minor, payment.amount_minor)?;
            }
        }
    }

    // Only cash can be over-tendered — it is the only tender that gives change
    // back. A card slip and a transfer to the account both settle exactly.
    let non_cash_minor = checked_add(card_minor, bank_transfer_minor)?;
    let tendered_minor = checked_add(cash_received_minor, non_cash_minor)?;

    if non_cash_minor > total_minor || tendered_minor < total_minor {
        return Err(AppError::business(
            "payment_mismatch",
            "Plaćanja se ne poklapaju sa ukupnim iznosom.",
        ));
    }

    Ok(PaymentAllocation {
        cash_received_minor,
        change_due_minor: tendered_minor - total_minor,
        stored_cash_minor: total_minor - non_cash_minor,
        stored_card_minor: card_minor,
        stored_bank_transfer_minor: bank_transfer_minor,
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

    if payment.stored_bank_transfer_minor > 0 {
        payments.push(PaymentDraft {
            method: PaymentMethod::BankTransfer,
            amount_minor: payment.stored_bank_transfer_minor,
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
            AppError::InvalidState(format!("Podešavanja računa nisu ispravna: {source}"))
        })?,
        None => ReceiptSettings::default(),
    };
    let receipt_number = format!("{}{:06}", settings.prefix, settings.next_sequence_number);
    let next_settings = ReceiptSettings {
        next_sequence_number: settings.next_sequence_number + 1,
        ..settings
    };
    let value_json = serde_json::to_string(&next_settings).map_err(|source| {
        AppError::InvalidState(format!("Podešavanja računa nisu ispravna: {source}"))
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
                    "Procenat popusta mora biti između 0 i 100%.",
                ));
            }

            rounded_div(total_minor as i128 * *basis_points as i128, 10_000)?
        }
    };

    if amount > total_minor {
        return Err(AppError::business(
            "invalid_discount",
            "Popust ne može biti veći od iznosa.",
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
            "Obračun prodaje nije ispravan.".to_string(),
        ));
    }

    let value = (numerator + denominator / 2) / denominator;
    i64::try_from(value)
        .map_err(|_| AppError::InvalidState("Obračun prodaje je prevelik za upis.".to_string()))
}

fn checked_add(left: i64, right: i64) -> Result<i64, AppError> {
    left.checked_add(right)
        .ok_or_else(|| AppError::InvalidState("Obračun prodaje je prevelik za upis.".to_string()))
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
        assess_draft_price_integrity, build_sale_preview, complete_sale_transaction,
        CompleteSaleRequest, DiscountRequest, PaymentDraft, PaymentMethod, SaleDraftItem,
        SaleDraftRequest,
    };
    use crate::commands::settings::{
        save_eur_rate, save_shop_profile, PravnaForma, ShopProfile, ShopProfileRequest,
        EUR_RATE_KEY, SHOP_PROFILE_KEY,
    };
    use crate::db::{test_database_path, Db};
    use crate::nbs_rate::{EurRate, RateSource};
    use crate::state::AppState;

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

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) {
        let admin_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist");
        state
            .set_session_user_id(admin_id)
            .expect("admin session should set");
    }

    /// The product is priced at 1000 para (10 RSD) per unit, which makes the
    /// line total in para equal to `quantity_milli` — so a test can ask for an
    /// exact dinar amount without doing arithmetic in the assertion.
    fn seed_admin_shift_and_product(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        let admin_id: i64 = connection
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist");

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
                 VALUES (?1, '2026-07-31T09:00:00Z', 0, 0, 'open', '2026-07-31T09:00:00Z', '2026-07-31T09:00:00Z')",
                params![admin_id],
            )
            .expect("shift should insert");

        connection
            .execute(
                "INSERT INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES ('PDV 20', 2000, '2026-07-31T09:00:00Z', '2026-07-31T09:00:00Z')",
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
                 VALUES ('Šporet', 'SPORET-1', '8600000000027', 'kom', 1000, 800, ?1, 0, 0, 1, '2026-07-31T09:00:00Z', '2026-07-31T09:00:00Z')",
                params![tax_rate_id],
            )
            .expect("product should insert");
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                 VALUES (?1, 100000000, '2026-07-31T09:00:00Z')",
                params![product_id],
            )
            .expect("inventory balance should insert");

        product_id
    }

    fn draft_item_worth(product_id: i64, total_minor: i64) -> SaleDraftItem {
        SaleDraftItem {
            product_id,
            quantity_milli: total_minor,
            discount: None,
        }
    }

    fn preduzetnik_profile_request() -> ShopProfileRequest {
        ShopProfileRequest {
            pravna_forma: Some(PravnaForma::Preduzetnik),
            pdv_obveznik: Some(false),
            distance_selling: Some(false),
            lpfr_in_premises: Some(true),
            lpfr_carve_out_internet_only: Some(false),
            lpfr_carve_out_own_used_assets: Some(false),
            esir_elements: Vec::new(),
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
            aml_ack_reason: None,
        };

        let completed =
            complete_sale_transaction(&seeded.db, request, None).expect("sale should complete");

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
            aml_ack_reason: None,
        };

        let error = complete_sale_transaction(&seeded.db, request, None)
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
            aml_ack_reason: None,
        };

        let error = complete_sale_transaction(&seeded.db, request, None)
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
            aml_ack_reason: None,
        };

        complete_sale_transaction(&seeded.db, request, None)
            .expect("oversell override should succeed");

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
            aml_ack_reason: None,
        };

        complete_sale_transaction(&seeded.db, request, None)
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
            aml_ack_reason: None,
        };

        let error = complete_sale_transaction(&seeded.db, request, None)
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
            aml_ack_reason: None,
        };

        complete_sale_transaction(&seeded.db, request, None).expect("sale should complete");

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
            aml_ack_reason: None,
        };

        let completed =
            complete_sale_transaction(&seeded.db, request, None).expect("sale should complete");

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
            aml_ack_reason: None,
        };

        let error = complete_sale_transaction(&seeded.db, request, None)
            .expect_err("card overpayment should fail");

        assert_eq!(error.code(), "payment_mismatch");

        let _ = std::fs::remove_file(seeded.db_path);
    }

    #[test]
    fn completing_a_sale_persists_the_aml_decision_when_the_cap_is_reached() {
        with_state("aml_persisted_on_sale", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");
            // threshold = 10_000 * 100 = 1_000_000 para

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 1_000_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 1_000_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: Some("Kupac odbio prenos na račun".to_string()),
                },
                None,
            )
            .expect("sale should complete — the warning is soft, never a block");

            let conn = state.db().open().expect("db opens");
            let (cash, rate, date, source, reason): (i64, i64, String, String, String) = conn
                .query_row(
                    "SELECT aml_cash_minor, aml_rate_minor, aml_rate_date, aml_rate_source, aml_ack_reason
                     FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .expect("AML provenance must be reproducible at inspection");

            assert_eq!(cash, 1_000_000);
            assert_eq!(rate, 100);
            assert_eq!(date, "2026-07-31");
            assert_eq!(source, "nbs");
            assert_eq!(reason, "Kupac odbio prenos na račun");
        });
    }

    #[test]
    fn an_ordinary_sale_leaves_the_aml_columns_null() {
        with_state("aml_absent_on_ordinary_sale", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 50_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 50_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect("sale completes");

            let conn = state.db().open().expect("db opens");
            let cash: Option<i64> = conn
                .query_row(
                    "SELECT aml_cash_minor FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| row.get(0),
                )
                .expect("row exists");
            assert_eq!(
                cash, None,
                "no assessment fired, so no provenance is written"
            );
        });
    }

    fn compliance_log_count(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("db opens")
            .query_row("SELECT COUNT(*) FROM compliance_log", [], |row| row.get(0))
            .expect("compliance_log should count")
    }

    fn admin_id(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("db opens")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist")
    }

    /// §3 req 8 [PRUDENTIAL]: the soft block on the čl. 46 st. 1 cash cap owes
    /// an *immutable audit entry*, not merely the columns on the sale row. The
    /// entry has to carry enough to reproduce the decision at inspection —
    /// which sale, which receipt, how much cash stayed in the drawer, the
    /// threshold it was measured against, the rate that produced the threshold
    /// with its date and source, and what the operator gave as the reason.
    #[test]
    fn a_breaching_sale_writes_one_immutable_aml_audit_entry() {
        with_state("aml_audit_entry_on_breach", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");
            // threshold = 10_000 * 100 = 1_000_000 para

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 1_000_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 1_000_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: Some("Kupac odbio prenos na račun".to_string()),
                },
                Some(admin_id(state)),
            )
            .expect("the block is soft — the sale still completes");

            assert_eq!(
                compliance_log_count(state),
                1,
                "exactly one audit entry per breach"
            );

            let conn = state.db().open().expect("db opens");
            let (event_type, detail, user_id, created_at): (String, String, i64, String) = conn
                .query_row(
                    "SELECT event_type, detail_json, user_id, created_at FROM compliance_log",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("the audit entry should be readable");

            assert_eq!(event_type, "aml_cash_threshold");
            assert_eq!(user_id, admin_id(state), "the acting session user");
            assert_eq!(
                created_at, sale.created_at,
                "the entry carries the sale's own timestamp — the two records \
                 must not be able to disagree about when the cash was accepted"
            );

            let detail: serde_json::Value =
                serde_json::from_str(&detail).expect("detail_json should parse");
            assert_eq!(detail["sale_id"], serde_json::json!(sale.id));
            assert_eq!(
                detail["local_receipt_number"],
                serde_json::json!(sale.local_receipt_number)
            );
            assert_eq!(detail["cash_minor"], serde_json::json!(1_000_000));
            assert_eq!(detail["threshold_minor"], serde_json::json!(1_000_000));
            assert_eq!(detail["rate_minor"], serde_json::json!(100));
            assert_eq!(detail["rate_date"], serde_json::json!("2026-07-31"));
            assert_eq!(detail["rate_source"], serde_json::json!("nbs"));
            assert_eq!(
                detail["ack_reason"],
                serde_json::json!("Kupac odbio prenos na račun")
            );
            assert_eq!(
                detail["note"],
                serde_json::json!(
                    "Gotovina zadržana u iznosu na ili iznad praga iz čl. 46 st. 1 \
                     Zakona o sprečavanju pranja novca i finansiranja terorizma."
                ),
                "the note keeps its diacritics and reads as one sentence"
            );
        });
    }

    /// The soft line is a [PRUDENTIAL] early warning, not a breach of čl. 46
    /// st. 1 — nothing below the cap is unlawful. It fills the sale's
    /// provenance columns so the till can show what it warned about, but it
    /// must not manufacture a compliance event for a lawful sale.
    #[test]
    fn a_near_threshold_sale_writes_no_aml_audit_entry() {
        with_state("aml_audit_silent_near_threshold", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");
            // threshold = 1_000_000 para; the soft line sits at 800_000.

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 999_999)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 999_999,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: Some("Blizu praga".to_string()),
                },
                Some(admin_id(state)),
            )
            .expect("sale completes");

            let conn = state.db().open().expect("db opens");
            let cash: Option<i64> = conn
                .query_row(
                    "SELECT aml_cash_minor FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| row.get(0),
                )
                .expect("row exists");
            assert_eq!(
                cash,
                Some(999_999),
                "the warning did fire, so the sale keeps its provenance"
            );

            assert_eq!(
                compliance_log_count(state),
                0,
                "one para below the cap is lawful — a warning is not a breach"
            );
        });
    }

    #[test]
    fn an_ordinary_sale_writes_no_aml_audit_entry() {
        with_state("aml_audit_silent_ordinary_sale", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");

            complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 50_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 50_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                Some(admin_id(state)),
            )
            .expect("sale completes");

            assert_eq!(
                compliance_log_count(state),
                0,
                "an ordinary sale leaves no compliance event"
            );
        });
    }

    /// The audit entry lives in the **sale's own transaction**: a breaching
    /// sale can never exist without its entry, nor the entry without its sale.
    ///
    /// Two legs, because they fail at different points of the write:
    ///
    /// * insufficient stock is refused *before* the assessment runs, so it
    ///   guards against the audit write ever being hoisted above the stock
    ///   gate;
    /// * the injected fault aborts *after* the audit row is already in the
    ///   transaction, which is the case that actually proves the boundary. A
    ///   plain `INSERT` outside the transaction would survive it and leave an
    ///   orphan event describing a sale that never happened.
    #[test]
    fn the_aml_audit_entry_shares_the_sales_transaction() {
        with_state("aml_audit_entry_is_atomic", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");
            // threshold = 1_000_000 para; the seeded balance is 100_000_000.

            let breaching_sale = |quantity_milli: i64| CompleteSaleRequest {
                items: vec![draft_item_worth(product_id, quantity_milli)],
                receipt_discount: None,
                payments: vec![PaymentDraft {
                    method: PaymentMethod::Cash,
                    amount_minor: quantity_milli,
                }],
                allow_stock_override: None,
                aml_ack_reason: Some("Kupac odbio prenos na račun".to_string()),
            };

            let error = complete_sale_transaction(
                state.db(),
                breaching_sale(200_000_000),
                Some(admin_id(state)),
            )
            .expect_err("overselling is disabled, so the sale must be refused");
            assert_eq!(error.code(), "insufficient_stock");
            assert_eq!(
                compliance_log_count(state),
                0,
                "a refused sale leaves no compliance event"
            );

            {
                let conn = state.db().open().expect("db opens");
                conn.execute_batch(
                    "CREATE TRIGGER fail_after_the_audit_write
                     BEFORE INSERT ON sale_items
                     BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
                )
                .expect("the fault trigger should install");
            }

            complete_sale_transaction(state.db(), breaching_sale(1_000_000), Some(admin_id(state)))
                .expect_err("the injected fault must fail the sale");

            let conn = state.db().open().expect("db opens");
            let sales: i64 = conn
                .query_row("SELECT COUNT(*) FROM sales", [], |row| row.get(0))
                .expect("sales should count");
            assert_eq!(sales, 0, "the sale rolled back");
            assert_eq!(
                compliance_log_count(state),
                0,
                "the audit entry rolled back with it — no orphan event"
            );
        });
    }

    /// §3 req 1 [LEGAL]: čl. 46 st. 1 keys to the **cash line** of the split,
    /// never the grand total. This sale's total sits exactly on the cap, but
    /// only 300.000 para of it is cash — below even the soft line — so nothing
    /// fires and no provenance is written. Assessing
    /// `computation.preview.total_minor` here would invent a breach that the
    /// statute does not describe.
    #[test]
    fn completion_assesses_the_cash_line_not_the_invoice_total() {
        with_state("aml_completion_reads_the_cash_line", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");
            // threshold = 10_000 * 100 = 1_000_000 para; soft line = 800_000.

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 1_000_000)],
                    receipt_discount: None,
                    payments: vec![
                        PaymentDraft {
                            method: PaymentMethod::Card,
                            amount_minor: 700_000,
                        },
                        PaymentDraft {
                            method: PaymentMethod::Cash,
                            amount_minor: 300_000,
                        },
                    ],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect("sale completes");

            let conn = state.db().open().expect("db opens");
            let cash: Option<i64> = conn
                .query_row(
                    "SELECT aml_cash_minor FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| row.get(0),
                )
                .expect("row exists");

            assert_eq!(
                cash, None,
                "§3 req 1: the invoice total is on the cap but the cash line is \
                 300.000 para — assessing the total would fire a false breach"
            );
        });
    }

    /// A verdict never rejects the sale (§3 req 1). A settings row that will not
    /// deserialize must therefore degrade to "no rate, no profile" — the
    /// `rate_unavailable` path — rather than leaving the register unable to sell
    /// anything.
    #[test]
    fn a_corrupt_settings_row_never_blocks_the_till() {
        with_state("aml_corrupt_settings", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);

            {
                let conn = state.db().open().expect("db opens");
                for key in [EUR_RATE_KEY, SHOP_PROFILE_KEY] {
                    conn.execute(
                        "INSERT OR REPLACE INTO settings (key, value_json, updated_at)
                         VALUES (?1, '{not json', '2026-07-31T09:00:00Z')",
                        params![key],
                    )
                    .expect("corrupt setting should insert");
                }
            }

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 5_000_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 5_000_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect("a corrupt settings row must not turn the till into a dead register");

            let conn = state.db().open().expect("db opens");
            let cash: Option<i64> = conn
                .query_row(
                    "SELECT aml_cash_minor FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| row.get(0),
                )
                .expect("row exists");
            assert_eq!(
                cash, None,
                "without a usable rate nothing fires, so nothing is persisted"
            );
        });
    }

    #[test]
    fn mixed_tender_below_the_cap_in_cash_does_not_breach() {
        let rate = EurRate {
            rate_minor: 100,
            rate_date: "2026-07-31".to_string(),
            source: RateSource::Nbs,
        };
        let profile = ShopProfile {
            pravna_forma: Some(PravnaForma::Preduzetnik),
            ..ShopProfile::default()
        };

        // 1.500.000 total, but only 500.000 of it in cash.
        let assessment = crate::aml::assess_cash_payment(500_000, Some(&rate), &profile, 80);

        assert!(
            !assessment.breached,
            "cl. 46 st. 1 keys to the cash received, not the invoice"
        );
    }

    /// The lawful alternative čl. 46 st. 1 points the customer to is a transfer
    /// to the account. It is a tender of its own: stored under its own method,
    /// never added to the drawer, never given change, and — because it is not
    /// cash — never able to breach the cap. Rate 100 with a preduzetnik profile
    /// puts the threshold at exactly 1.000.000 para, so a transfer of that
    /// amount would breach if it were miscounted as cash.
    #[test]
    fn a_bank_transfer_is_its_own_tender_and_never_reaches_the_till() {
        with_state("bank_transfer_tender_end_to_end", |state| {
            let product_id = seed_admin_shift_and_product(state);
            sign_in_admin(state);
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 100,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate saves");

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 1_000_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::BankTransfer,
                        amount_minor: 1_000_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect("a transfer to the account is a valid tender");

            assert_eq!(sale.cash_received_minor, 0);
            assert_eq!(sale.change_due_minor, 0, "a transfer never gives change");
            assert_eq!(sale.payments.len(), 1, "the receipt shows one tender");
            assert_eq!(sale.payments[0].method, PaymentMethod::BankTransfer);
            assert_eq!(sale.payments[0].amount_minor, 1_000_000);

            let conn = state.db().open().expect("db opens");
            let (method, amount): (String, i64) = conn
                .query_row(
                    "SELECT payment_method, amount_minor FROM sale_payments WHERE sale_id = ?1",
                    params![sale.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the tender is stored");
            assert_eq!(method, "bank_transfer");
            assert_eq!(amount, 1_000_000);

            let expected_cash_minor: i64 = conn
                .query_row(
                    "SELECT expected_cash_minor FROM shifts WHERE status = 'open'",
                    [],
                    |row| row.get(0),
                )
                .expect("the open shift is readable");
            assert_eq!(expected_cash_minor, 0, "a transfer never enters the drawer");

            let aml_cash: Option<i64> = conn
                .query_row(
                    "SELECT aml_cash_minor FROM sales WHERE id = ?1",
                    params![sale.id],
                    |row| row.get(0),
                )
                .expect("row exists");
            assert_eq!(
                aml_cash, None,
                "the cap keys to cash — the lawful alternative can never breach it"
            );
        });
    }

    /// A split that settles part in cash and part by transfer still owes no
    /// change, stores both tenders, and puts only the cash line in the drawer.
    #[test]
    fn a_cash_and_bank_transfer_split_stores_both_tenders() {
        with_state("bank_transfer_split_tender", |state| {
            let product_id = seed_admin_shift_and_product(state);

            let sale = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 1_000_000)],
                    receipt_discount: None,
                    payments: vec![
                        PaymentDraft {
                            method: PaymentMethod::BankTransfer,
                            amount_minor: 700_000,
                        },
                        PaymentDraft {
                            method: PaymentMethod::Cash,
                            amount_minor: 300_000,
                        },
                    ],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect("a split with a transfer completes");

            assert_eq!(sale.change_due_minor, 0);

            let conn = state.db().open().expect("db opens");
            let mut statement = conn
                .prepare(
                    "SELECT payment_method, amount_minor FROM sale_payments
                     WHERE sale_id = ?1 ORDER BY payment_method",
                )
                .expect("statement prepares");
            let stored: Vec<(String, i64)> = statement
                .query_map(params![sale.id], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("payments query")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("payments collect");
            assert_eq!(
                stored,
                vec![
                    ("bank_transfer".to_string(), 700_000),
                    ("cash".to_string(), 300_000),
                ]
            );

            let expected_cash_minor: i64 = conn
                .query_row(
                    "SELECT expected_cash_minor FROM shifts WHERE status = 'open'",
                    [],
                    |row| row.get(0),
                )
                .expect("the open shift is readable");
            assert_eq!(
                expected_cash_minor, 300_000,
                "only the cash line is in the till"
            );
        });
    }

    /// A transfer is a non-cash tender, so — like a card — it may not exceed the
    /// receipt total: there is no change to give back on it.
    #[test]
    fn a_bank_transfer_above_the_total_is_a_payment_mismatch() {
        with_state("bank_transfer_overpayment", |state| {
            let product_id = seed_admin_shift_and_product(state);

            let error = complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: vec![draft_item_worth(product_id, 100_000)],
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::BankTransfer,
                        amount_minor: 120_000,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                None,
            )
            .expect_err("a transfer overpayment must be refused");

            assert_eq!(error.code(), "payment_mismatch");
        });
    }

    // ---------------------------------------------------------------------
    // Task 5 — the till-side price-integrity guard (req. 12, čl. 6 st. 4)
    // ---------------------------------------------------------------------

    /// A till whose outlet has published a cenovnik: the seeded product at
    /// 1000 para, an open shift, and one snapshot rendered from the catalog as it
    /// then stood. Returns the product id and the snapshot the guard will name.
    fn seed_published_till(state: &AppState) -> (i64, i64) {
        let product_id = seed_admin_shift_and_product(state);
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO settings (key, value_json, updated_at)
                 VALUES ('company', ?1, '2026-07-31T09:00:00Z')
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![serde_json::json!({
                    "shopName": "Butik Ana",
                    "address": "Bulevar oslobođenja 1, Novi Sad",
                    "pib": "",
                    "registrationNumber": "",
                    "phone": "",
                    "logoPath": null,
                    "currency": "RSD",
                })
                .to_string()],
            )
            .expect("company settings should save");

        let snapshot_id = crate::commands::cenovnik::publish_current(
            &connection,
            &crate::cenovnik::NotConfigured,
            "2026-07-31T09:30:00Z",
        )
        .expect("the outlet should publish")
        .expect("an identified outlet publishes");

        (product_id, snapshot_id)
    }

    /// Moves the catalog price without republishing — the state čl. 6 st. 3 calls
    /// a stale cenovnik, and the only one in which the till can charge above what
    /// the shop published.
    fn move_the_shelf_price_without_republishing(state: &AppState, product_id: i64, minor: i64) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "UPDATE products SET sale_price_minor = ?1 WHERE id = ?2",
                params![minor, product_id],
            )
            .expect("the shelf price should move");
    }

    fn cash_sale(product_id: i64, quantity_milli: i64, cash_minor: i64) -> CompleteSaleRequest {
        CompleteSaleRequest {
            items: vec![SaleDraftItem {
                product_id,
                quantity_milli,
                discount: None,
            }],
            receipt_discount: None,
            payments: vec![PaymentDraft {
                method: PaymentMethod::Cash,
                amount_minor: cash_minor,
            }],
            allow_stock_override: None,
            aml_ack_reason: None,
        }
    }

    /// Req. 12. Čl. 6 st. 4 binds a trader who publishes a cenovnik to adhere to
    /// the prices in it, so ringing an article above the published price is the
    /// one direction the till has to say something about — and it says it, it does
    /// not refuse: the register must be able to record what actually happened.
    #[test]
    fn ringing_above_the_published_price_raises_a_divergence() {
        with_state("cenovnik_guard_above_published", |state| {
            let (product_id, snapshot_id) = seed_published_till(state);
            move_the_shelf_price_without_republishing(state, product_id, 1_500);

            let divergences =
                assess_draft_price_integrity(state.db(), sale_draft(product_id, 1_000))
                    .expect("the guard should assess");
            assert_eq!(divergences.len(), 1, "{divergences:?}");
            assert_eq!(divergences[0].product_id, product_id);
            assert_eq!(divergences[0].product_sku, "SPORET-1");
            assert_eq!(divergences[0].charged_unit_price_minor, 1_500);
            assert_eq!(divergences[0].published_unit_price_minor, 1_000);
            assert_eq!(divergences[0].snapshot_id, snapshot_id);
            assert_eq!(divergences[0].snapshot_generated_at, "2026-07-31T09:30:00Z");

            complete_sale_transaction(state.db(), cash_sale(product_id, 1_000, 1_500), None)
                .expect("the guard warns — it never refuses the sale");
        });
    }

    /// A discount is not a breach: čl. 6 st. 4 binds the shop to prices it must
    /// not exceed, not to prices it must charge. Only the above direction matters,
    /// which is also what keeps the guard to a single comparison.
    #[test]
    fn ringing_below_the_published_price_is_silent() {
        with_state("cenovnik_guard_below_published", |state| {
            let (product_id, _) = seed_published_till(state);
            move_the_shelf_price_without_republishing(state, product_id, 700);

            assert!(
                assess_draft_price_integrity(state.db(), sale_draft(product_id, 1_000))
                    .expect("the guard should assess")
                    .is_empty(),
                "a price below the published one is a discount, not a divergence"
            );

            let sale = complete_sale_transaction(
                state.db(),
                cash_sale(product_id, 1_000, 700),
                Some(admin_id(state)),
            )
            .expect("the sale should complete");
            assert_eq!(sale.total_minor, 700);
            assert_eq!(
                compliance_log_count(state),
                0,
                "nothing to record: the shop charged less than it published"
            );
        });
    }

    /// Charging exactly the published price is adherence, not divergence — the
    /// comparison is strictly greater-than, and the boundary is the one place a
    /// guard written with `>=` would accuse a shop that did everything right.
    #[test]
    fn ringing_exactly_the_published_price_is_silent() {
        with_state("cenovnik_guard_at_published", |state| {
            let (product_id, _) = seed_published_till(state);

            assert!(
                assess_draft_price_integrity(state.db(), sale_draft(product_id, 1_000))
                    .expect("the guard should assess")
                    .is_empty()
            );
        });
    }

    /// Čl. 6 st. 1 is about the article's prodajna cena, and a discount is a
    /// reduction granted on that price rather than a different price for the
    /// article. So the comparison stays on the offered unit price — otherwise a
    /// discount line is the obvious way to ring above the published cenovnik
    /// unremarked.
    #[test]
    fn a_line_discount_does_not_hide_a_price_above_the_published_one() {
        with_state("cenovnik_guard_discount_does_not_hide", |state| {
            let (product_id, _) = seed_published_till(state);
            move_the_shelf_price_without_republishing(state, product_id, 1_500);

            let discounted = SaleDraftRequest {
                items: vec![SaleDraftItem {
                    product_id,
                    quantity_milli: 1_000,
                    // Brings the line back to the published 1000 para.
                    discount: Some(DiscountRequest::Amount { amount_minor: 500 }),
                }],
                receipt_discount: None,
            };

            let divergences = assess_draft_price_integrity(state.db(), discounted)
                .expect("the guard should assess");
            assert_eq!(divergences.len(), 1, "{divergences:?}");
            assert_eq!(divergences[0].charged_unit_price_minor, 1_500);
            assert_eq!(divergences[0].published_unit_price_minor, 1_000);
        });
    }

    /// The record has to name the exhibit. An outlet's archive holds many files
    /// and čl. 6 st. 4 binds the shop only to the one in force when the sale
    /// happened, so the entry carries that snapshot's id, its date and its hash —
    /// enough to pull the exact file back out of the archive and read the price
    /// the till was measured against.
    #[test]
    fn the_divergence_is_logged_with_the_snapshot_it_was_compared_against() {
        with_state("cenovnik_guard_logs_the_snapshot", |state| {
            let (product_id, snapshot_id) = seed_published_till(state);
            sign_in_admin(state);
            move_the_shelf_price_without_republishing(state, product_id, 1_500);

            let sale = complete_sale_transaction(
                state.db(),
                cash_sale(product_id, 1_000, 1_500),
                Some(admin_id(state)),
            )
            .expect("the guard never blocks the sale");

            assert_eq!(compliance_log_count(state), 1, "one entry per article");
            let connection = state.db().open().expect("db opens");
            let (event_type, detail, user_id, created_at): (String, String, i64, String) =
                connection
                    .query_row(
                        "SELECT event_type, detail_json, user_id, created_at FROM compliance_log",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .expect("the divergence entry should be readable");

            assert_eq!(event_type, "cenovnik_price_divergence");
            assert_eq!(user_id, admin_id(state), "the acting session user");
            assert_eq!(
                created_at, sale.created_at,
                "the entry carries the sale's own timestamp — the two records must \
                 not be able to disagree about when the price was charged"
            );

            let detail: serde_json::Value =
                serde_json::from_str(&detail).expect("detail_json should parse");
            assert_eq!(detail["sale_id"], serde_json::json!(sale.id));
            assert_eq!(
                detail["local_receipt_number"],
                serde_json::json!(sale.local_receipt_number)
            );
            assert_eq!(detail["product_id"], serde_json::json!(product_id));
            assert_eq!(detail["sifra"], serde_json::json!("SPORET-1"));
            assert_eq!(detail["naplacena_cena_minor"], serde_json::json!(1_500));
            assert_eq!(detail["objavljena_cena_minor"], serde_json::json!(1_000));
            assert_eq!(detail["snapshot_id"], serde_json::json!(snapshot_id));
            assert_eq!(
                detail["snapshot_generated_at"],
                serde_json::json!("2026-07-31T09:30:00Z")
            );

            // The hash pins WHICH bytes: an archive read back through the id alone
            // could not tell the reader that the file it found is the file the
            // comparison used.
            let published_hash: String = connection
                .query_row(
                    "SELECT content_hash FROM cenovnik_snapshots WHERE id = ?1",
                    params![snapshot_id],
                    |row| row.get(0),
                )
                .expect("the snapshot should read");
            assert_eq!(
                detail["snapshot_content_hash"],
                serde_json::json!(published_hash)
            );
        });
    }

    /// A shop that has published nothing has made no čl. 6 st. 4 promise to
    /// depart from — and §2b leaves it genuinely unresolved whether it must
    /// publish at all. So there is no guard, no entry, and above all no block.
    #[test]
    fn no_published_snapshot_means_no_guard_and_no_block() {
        with_state("cenovnik_guard_without_a_snapshot", |state| {
            let product_id = seed_admin_shift_and_product(state);
            move_the_shelf_price_without_republishing(state, product_id, 99_900);

            assert!(
                assess_draft_price_integrity(state.db(), sale_draft(product_id, 1_000))
                    .expect("an empty archive is not an error")
                    .is_empty()
            );

            let sale = complete_sale_transaction(
                state.db(),
                cash_sale(product_id, 1_000, 99_900),
                Some(admin_id(state)),
            )
            .expect("a shop that has published nothing must still be able to sell");
            assert_eq!(sale.total_minor, 99_900);
            assert_eq!(compliance_log_count(state), 0);
        });
    }

    /// An article the published file does not carry — created after the last
    /// republish — has no published price, and a guard that treated „absent“ as
    /// „zero“ would accuse the shop of a divergence on every sale of it.
    #[test]
    fn an_article_the_published_file_does_not_carry_raises_no_divergence() {
        with_state("cenovnik_guard_unpublished_article", |state| {
            let (_, _) = seed_published_till(state);
            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO products (
                        name, sku, unit_of_measure, sale_price_minor, purchase_price_minor,
                        tax_rate_id, minimum_stock_milli, allow_negative_stock, active,
                        created_at, updated_at
                     )
                     VALUES ('Novi artikal', 'NOVI-1', 'kom', 5000, 3000,
                             (SELECT id FROM tax_rates LIMIT 1), 0, 1, 1,
                             '2026-08-02T09:00:00Z', '2026-08-02T09:00:00Z')",
                    [],
                )
                .expect("a product created after the last republish should insert");
            let unpublished = connection.last_insert_rowid();

            assert!(
                assess_draft_price_integrity(state.db(), sale_draft(unpublished, 1_000))
                    .expect("the guard should assess")
                    .is_empty(),
                "an article with no published price cannot diverge from one"
            );
        });
    }

    /// One article rung twice is one divergence: the unit price comes from the
    /// catalog, so both lines carry the same figure, and two identical entries
    /// would be two accusations over one departure from one published price.
    #[test]
    fn one_article_rung_on_two_lines_is_one_divergence() {
        with_state("cenovnik_guard_dedupes_per_article", |state| {
            let (product_id, _) = seed_published_till(state);
            sign_in_admin(state);
            move_the_shelf_price_without_republishing(state, product_id, 1_500);

            let two_lines = SaleDraftRequest {
                items: vec![
                    SaleDraftItem {
                        product_id,
                        quantity_milli: 1_000,
                        discount: None,
                    },
                    SaleDraftItem {
                        product_id,
                        quantity_milli: 2_000,
                        discount: None,
                    },
                ],
                receipt_discount: None,
            };
            assert_eq!(
                assess_draft_price_integrity(state.db(), two_lines.clone())
                    .expect("the guard should assess")
                    .len(),
                1
            );

            complete_sale_transaction(
                state.db(),
                CompleteSaleRequest {
                    items: two_lines.items,
                    receipt_discount: None,
                    payments: vec![PaymentDraft {
                        method: PaymentMethod::Cash,
                        amount_minor: 4_500,
                    }],
                    allow_stock_override: None,
                    aml_ack_reason: None,
                },
                Some(admin_id(state)),
            )
            .expect("the sale should complete");
            assert_eq!(compliance_log_count(state), 1);
        });
    }

    /// „Warn, never block“ in its strongest form. An archive the guard cannot read
    /// is a defect in an auxiliary table, and čl. 6 is not a reason a till cannot
    /// sell — so the guard degrades to silence and the sale goes through.
    #[test]
    fn an_unreadable_archive_silences_the_guard_instead_of_stopping_the_till() {
        with_state("cenovnik_guard_unreadable_archive", |state| {
            let (product_id, _) = seed_published_till(state);
            move_the_shelf_price_without_republishing(state, product_id, 1_500);
            state
                .db()
                .open()
                .expect("database should open")
                .execute("DROP TABLE cenovnik_snapshots", [])
                .expect("the archive should drop");

            assert!(
                assess_draft_price_integrity(state.db(), sale_draft(product_id, 1_000))
                    .expect("an unreadable archive is not an error at the till")
                    .is_empty()
            );

            let sale = complete_sale_transaction(
                state.db(),
                cash_sale(product_id, 1_000, 1_500),
                Some(admin_id(state)),
            )
            .expect("a broken archive must never stop the register");
            assert_eq!(sale.total_minor, 1_500);
            assert_eq!(compliance_log_count(state), 0);
        });
    }
}
