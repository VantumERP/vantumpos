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
    pub purchase_price_minor: Option<i64>,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    /// The supplier's isprava o nabavci this delivery came with (ZoT čl. 29
    /// st. 1). Optional: the receive path warns on a missing one and never
    /// blocks. Read only on a `Receive`; a correction or a write-off is not a
    /// nabavka and has no dobavljač.
    #[serde(default)]
    pub isprava_id: Option<i64>,
}

/// ZoT čl. 34 st. 1–2 puts the marking duty on the proizvođač/uvoznik, but čl. 68
/// st. 1 tač. 9 punishes the *trgovac* who sells goods without a deklaracija. The
/// goods receipt is the last moment the shop can refuse the pallet, so this is
/// where the operator is told — and only told. Blocking the receipt would strand
/// stock the shop already physically holds and is not what the law asks for.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationWarning {
    pub product_id: i64,
    pub product_name: String,
    /// camelCase field names, so the UI can point at the very input that is blank.
    pub missing_fields: Vec<String>,
    /// What the shop is actually looking at: a gap in its own records, not a
    /// proven offence. The čl. 34 st. 1 data lives on the packaging, so blank
    /// catalog fields say nothing about the pallet in the stockroom. `notice`
    /// below is the exposure this gap leaves *unverified* — the UI must render
    /// this line with it, never the notice alone (§3 req 26 `[ACCURACY]`).
    pub advisory: String,
    pub notice: crate::legal::LegalNotice,
}

/// Kept next to the struct so the qualifier can never drift away from the notice
/// it qualifies. Shared with the catalog deklaracija-gaps report, which shows the
/// same notice about the same two blank columns: two copies of this sentence would
/// let one surface soften the čl. 69a wording the other one hardened.
pub(crate) const DECLARATION_ADVISORY: &str = "U sistemu nisu evidentirani podaci sa deklaracije. \
     Ako roba fizički nosi ispravnu deklaraciju, prekršaja nema — unesite podatke sa \
     deklaracije ili evidentirajte proveru deklaracije (olakšavajuća okolnost, čl. 69a).";

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
    /// Advisory only. Always empty for corrections and write-offs — nothing new
    /// arrives through those doors.
    pub declaration_warnings: Vec<DeclarationWarning>,
    /// The ZoT čl. 29 st. 1 advisory, when the receipt was booked with no supplier
    /// isprava attached. **A separate field from `declaration_warnings` on
    /// purpose:** that one is about the manufacturer's čl. 34 marking on the
    /// goods, this one is about the document the trgovac must hold. Merging them
    /// would blur which of two different duties is unmet.
    pub isprava_warning: Option<IspravaWarning>,
}

/// Raised after a goods receipt is committed with no supplier isprava attached.
///
/// Advisory by construction, like `DeclarationWarning`: it is produced *after*
/// the movement, the kalkulacija and the zaduženje are written, so it cannot gate
/// them. PEP čl. 12 st. 1 has entries made „na osnovu verodostojnih isprava“,
/// which is worth saying out loud — but refusing the receipt would strand a
/// pallet the shop physically holds and would leave the goods unbooked, and
/// unbooked goods are the heavier offence tier.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IspravaWarning {
    /// What the shop is actually looking at — a receipt recorded with no supplier
    /// document behind it — as distinct from `notice`, which is the exposure that
    /// leaves unverified. The UI renders this line with the notice, never the
    /// notice alone.
    pub advisory: String,
    pub notice: crate::legal::LegalNotice,
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
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::Receive,
        request,
        acting_user_id,
        &created_at,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn inventory_correct(
    state: State<'_, AppState>,
    request: InventoryAdjustmentRequest,
) -> Result<InventoryAdjustmentResult, CommandError> {
    super::auth::require_admin(state.inner())?;
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::Correction,
        request,
        acting_user_id,
        &created_at,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn inventory_write_off(
    state: State<'_, AppState>,
    request: InventoryAdjustmentRequest,
) -> Result<InventoryAdjustmentResult, CommandError> {
    super::auth::require_admin(state.inner())?;
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(
        &mut connection,
        InventoryMovementType::WriteOff,
        request,
        acting_user_id,
        &created_at,
    )
    .map_err(Into::into)
}

/// ZoT čl. 34: the operator confirms by hand that the delivered goods carry a
/// deklaracija. The stamp is an audit record of *who looked and when* — it is not
/// an assertion that the catalog fields are transcribed, because the st. 1 data
/// lives on the packaging and the shop may lawfully hold goods it has not typed
/// in.
///
/// Session-gated, not admin-gated. The čl. 69a tač. 4 mitigation record belongs
/// where the goods are received, and `inventory_receive` admits the cashier; an
/// admin-only stamp would be reachable only by someone who is not at the pallet.
/// `mark_declaration_checked` derives the acting user from the session itself,
/// so the record still names whoever actually looked.
#[tauri::command]
pub fn inventory_mark_declaration_checked(
    state: State<'_, AppState>,
    product_id: i64,
) -> Result<(), CommandError> {
    super::auth::require_session(state.inner())?;
    let now = now_utc_string()?;
    mark_declaration_checked(state.inner(), product_id, &now).map_err(Into::into)
}

pub fn mark_declaration_checked(
    state: &AppState,
    product_id: i64,
    now: &str,
) -> Result<(), AppError> {
    let acting_user_id = super::auth::require_session(state)?;
    let connection = state.db().open()?;
    let updated = connection.execute(
        "UPDATE products
         SET declaration_checked_at = ?1,
             declaration_checked_by = ?2
         WHERE id = ?3",
        params![now, acting_user_id, product_id],
    )?;

    if updated == 0 {
        return Err(AppError::not_found("Artikal nije pronađen."));
    }

    Ok(())
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
    acting_user_id: i64,
    created_at: &str,
) -> Result<InventoryAdjustmentResult, AppError> {
    validate_adjustment_request(movement_type, &request)?;

    // ZoT cl. 37 st. 7: from the announcement of a rasprodaja until it ends, the
    // trader may not order and include new quantities of the goods that are its
    // subject. The ban is narrow: it applies to receiving stock of those SKUs
    // only. Corrections and write-offs stay allowed (counting and damage are not
    // "new quantities"), as does receiving any article outside the rasprodaja.
    if matches!(movement_type, InventoryMovementType::Receive) {
        let in_active_rasprodaja: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM campaign_items ci
                JOIN campaigns c ON c.id = ci.campaign_id
                WHERE ci.product_id = ?1
                  AND c.campaign_type = 'rasprodaja'
                  AND c.status = 'active'
             )",
            params![request.product_id],
            |row| row.get(0),
        )?;
        if in_active_rasprodaja {
            return Err(AppError::business(
                "rasprodaja_receive_blocked",
                "Artikal je predmet aktivne rasprodaje — prijem nove količine nije dozvoljen do kraja rasprodaje (čl. 37 st. 7).",
            ));
        }
    }

    let delta_quantity_milli = match movement_type {
        InventoryMovementType::Receive => request.quantity_milli,
        InventoryMovementType::Correction => request.quantity_milli,
        InventoryMovementType::WriteOff => request.quantity_milli.saturating_neg(),
    };
    let reason = normalized_optional_text(request.reason.as_deref());
    let reference_type = normalized_optional_text(request.reference_type.as_deref());
    let mut declaration_warnings = Vec::new();
    let mut isprava_warning = None;
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
            user_id: Some(acting_user_id),
            created_at,
            allow_overselling: false,
        },
    )?;

    // KEP evidencija prometa (SW-9a): only a goods receipt books a zaduženje, and
    // it is written inside this same transaction. The basis is retail value WITH
    // PDV (`quantity_milli * sale_price_minor / 1000`), NEVER the nabavna — that
    // is the memo's flagged false-assurance bug. Sharing `tx` makes the movement
    // and the ledger entry atomic: a rollback removes both, so goods can never
    // exist un-booked. Corrections and write-offs post nothing.
    if matches!(movement_type, InventoryMovementType::Receive) {
        let (sale_price_minor, nabavna_po_jm_minor): (i64, i64) = tx.query_row(
            "SELECT sale_price_minor, purchase_price_minor FROM products WHERE id = ?1",
            params![request.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // SW-9b: generate the kalkulacija (the receipt's formal isprava) FIRST,
        // then link the zaduženje to it (reference_type='kalkulacija'). Its
        // element 13 == qty × sale_price == the zaduženje amount, so the ledger
        // value is unchanged — 9b adds the isprava, not a re-post. All inside this
        // same transaction: a rollback removes the kalkulacija, the movement and
        // the ledger entry together.
        // ZoT čl. 29 st. 1: the supplier's isprava o nabavci, when the operator
        // attached one. Loaded inside the transaction so the opis and the
        // kalkulacija's link describe the very row that is being written, and so
        // an unknown id rolls the whole receipt back rather than booking goods
        // against a document that does not exist.
        let isprava = match request.isprava_id {
            Some(id) => Some(crate::dobavljaci::load_isprava_tx(&tx, id)?),
            None => None,
        };

        let kalkulacija = crate::kep_kalkulacija::create_kalkulacija(
            &tx,
            request.product_id,
            request.quantity_milli,
            nabavna_po_jm_minor,
            reference_type.as_deref(),
            request.reference_id,
            request.isprava_id,
            acting_user_id,
            created_at,
        )?;
        let opis = receipt_opis(
            reference_type.as_deref(),
            request.reference_id,
            reason.as_deref(),
            isprava.as_ref(),
            kalkulacija.redni_broj,
        );
        crate::kep::post_receipt_zaduzenje(
            &tx,
            request.product_id,
            request.quantity_milli,
            sale_price_minor,
            &opis,
            // PEP čl. 15 st. 3: kolona 2 carries the booking date, kolona 3 the
            // *document* date — „these are two different dates — never conflate
            // them.“ This argument has been None on every receipt until now
            // because no supplier document was captured to date it from.
            isprava.as_ref().map(|isprava| isprava.datum.as_str()),
            "kalkulacija",
            Some(kalkulacija.id),
            acting_user_id,
            created_at,
        )?;

        // The čl. 29 st. 1 advisory. Collected after the postings and returned,
        // never raised as an error: a receipt with no isprava is worth saying out
        // loud, but refusing it would strand a pallet the shop already holds and
        // would leave the goods unbooked, which is the worse tier.
        isprava_warning = missing_isprava_warning(&tx, isprava.as_ref());

        // SW-11c: the last moment the shop can still refuse the pallet. Collected
        // inside the transaction so the warning describes exactly the row that was
        // received. It sits after the zaduženje, so anything it returned as `Err`
        // would roll back the movement, the kalkulacija AND the ledger entry —
        // i.e. a hard block, which §3 req 12 forbids. Hence the infallible
        // signature: there is no `?` here to propagate.
        declaration_warnings = collect_declaration_warnings(&tx, request.product_id);
    }

    tx.commit()?;

    Ok(InventoryAdjustmentResult {
        product_id: request.product_id,
        movement_id: outcome.movement_id,
        movement_type: movement_type.as_str().to_string(),
        quantity_milli: delta_quantity_milli,
        previous_quantity_milli: outcome.previous_quantity_milli,
        new_quantity_milli: outcome.new_quantity_milli,
        created_at: created_at.to_string(),
        declaration_warnings,
        isprava_warning,
    })
}

/// A blank `manufacturer_name` or `country_of_origin` means the shop holds no
/// record of the ZoT čl. 34 st. 1 identity data for goods it just took in. That
/// is a warning and only a warning: čl. 68 st. 1 tač. 9 punishes *selling* such
/// goods, not receiving them, and a hard block would strand a pallet that is
/// already in the stockroom.
///
/// **Infallible by construction.** The caller runs this inside the receive
/// transaction, after the KEP zaduženje, so a `Result` here would be a hard block
/// wearing an advisory label — the exact thing §3 req 12 forbids. Every failure
/// mode therefore degrades instead of propagating:
///
/// - an unreadable `products` row yields no warning (the receipt is what matters);
/// - an unreadable `shop_profile` row falls back to `ShopProfile::default()`,
///   whose pravna forma is `None`, so the notice renders **no figure** rather than
///   a plausible one. `load_shop_profile_for_connection` keeps failing loudly for
///   the catalog čl. 34 st. 5 gate, where blocking is the correct answer; only
///   this advisory path absorbs it.
///
/// The notice is tier-resolved from the shop profile, so a shop whose pravna
/// forma is unset is shown the duty with no figure rather than a plausible one.
fn collect_declaration_warnings(
    connection: &Connection,
    product_id: i64,
) -> Vec<DeclarationWarning> {
    let row: Option<(String, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT name, manufacturer_name, country_of_origin FROM products WHERE id = ?1",
            params![product_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();

    let Some((product_name, manufacturer_name, country_of_origin)) = row else {
        return Vec::new();
    };

    let missing_fields = [
        (manufacturer_name, "manufacturerName"),
        (country_of_origin, "countryOfOrigin"),
    ]
    .into_iter()
    .filter(|(value, _)| value.as_deref().map(str::trim).unwrap_or("").is_empty())
    .map(|(_, field)| field.to_string())
    .collect::<Vec<_>>();

    if missing_fields.is_empty() {
        return Vec::new();
    }

    let profile = super::catalog::load_shop_profile_for_connection(connection).unwrap_or_default();

    vec![DeclarationWarning {
        product_id,
        product_name,
        missing_fields,
        advisory: DECLARATION_ADVISORY.to_string(),
        notice: crate::legal::declaration_missing(&profile),
    }]
}

/// What the shop is looking at when a receipt carries no isprava: a gap in its own
/// records. Kept next to the constructor so the qualifier cannot drift away from
/// the notice it qualifies.
const ISPRAVA_ADVISORY: &str = "Roba je primljena i evidentirana, ali uz prijem nije \
     vezana isprava dobavljača. Ako ispravu posedujete u papirnoj dokumentaciji, \
     evidentirajte je i povežite sa prijemom.";

/// `None` when an isprava was attached — there is nothing to warn about — and the
/// advisory otherwise.
///
/// **Infallible by construction**, for the same reason `collect_declaration_warnings`
/// is: the caller runs this inside the receive transaction *after* the kalkulacija
/// and the zaduženje, so a `Result` here would be a hard block wearing an advisory
/// label. An unreadable `shop_profile` falls back to `ShopProfile::default()`,
/// whose pravna forma is `None`, so the notice renders **no figure** rather than a
/// plausible one.
fn missing_isprava_warning(
    connection: &Connection,
    isprava: Option<&crate::dobavljaci::PrimljenaIsprava>,
) -> Option<IspravaWarning> {
    if isprava.is_some() {
        return None;
    }
    let profile = super::catalog::load_shop_profile_for_connection(connection).unwrap_or_default();
    Some(IspravaWarning {
        advisory: ISPRAVA_ADVISORY.to_string(),
        notice: crate::legal::isprava_missing(&profile),
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

/// The `opis` for a receipt zaduženje — KEP **kolona 3** (the previous comment
/// here said kolona 5; that is the razduženje column). Its prescribed content is
/// the document naziv, broj and datum, and for a nabavka additionally the
/// dobavljač: PEP čl. 15 st. 4, `docs/KEP-VERIFIED-RULES.md` §2.1.
///
/// With a supplier isprava attached, the composed line follows §2.7's worked
/// ledger row rather than a shape of our own invention:
///
/// ```text
/// Kalkulacija br. 12; otpremnica dobavljača „ABC d.o.o.“ br. 125 od 03.07.2026
/// ```
///
/// A natural-person supplier is named with prebivalište instead of poslovno ime,
/// and without the quotation marks — those belong to a poslovno ime. That branch
/// lives in `PrimljenaIsprava::kolona3_dobavljac`.
///
/// Without an isprava the pre-existing „Prijem robe“ text is returned unchanged.
/// That is deliberate: an unattached receipt should read as exactly what it is,
/// and dressing it up as though a document stood behind it would be the same
/// defect as a false tick in the compliance register.
fn receipt_opis(
    reference_type: Option<&str>,
    reference_id: Option<i64>,
    reason: Option<&str>,
    isprava: Option<&crate::dobavljaci::PrimljenaIsprava>,
    kalkulacija_redni_broj: i64,
) -> String {
    if let Some(isprava) = isprava {
        let dobavljac = if isprava.dobavljac_fizicko_lice {
            isprava.kolona3_dobavljac()
        } else {
            format!("„{}“", isprava.kolona3_dobavljac())
        };
        return format!(
            "Kalkulacija br. {kalkulacija_redni_broj}; {} dobavljača {dobavljac} br. {} od {}",
            isprava.vrsta,
            isprava.broj,
            format_datum_dmy(&isprava.datum),
        );
    }

    match (reference_type, reference_id) {
        (Some(ref_type), Some(ref_id)) => format!("Prijem robe ({ref_type} #{ref_id})"),
        (Some(ref_type), None) => format!("Prijem robe ({ref_type})"),
        (None, _) => match reason {
            Some(reason) => format!("Prijem robe — {reason}"),
            None => "Prijem robe".to_string(),
        },
    }
}

/// `2026-07-03` → `03.07.2026`, the form a Serbian document date is read in. A
/// value that is not the stored shape passes through rather than being mangled:
/// `dobavljaci::create_isprava` is what refuses a malformed datum, and repeating
/// the check here would put it in two places that could disagree.
fn format_datum_dmy(datum: &str) -> String {
    match (datum.get(0..4), datum.get(5..7), datum.get(8..10)) {
        (Some(year), Some(month), Some(day)) if datum.len() == 10 => {
            format!("{day}.{month}.{year}")
        }
        _ => datum.to_string(),
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
    use tauri::Manager;

    use crate::app_error::CommandError;
    use crate::commands::inventory::{
        apply_inventory_adjustment, get_product_ledger_for_connection, inventory_correct,
        inventory_mark_declaration_checked, inventory_receive, inventory_write_off,
        list_stock_for_connection, mark_declaration_checked, write_stock_movement,
        InventoryAdjustmentRequest, InventoryMovementType, StockListQuery, StockMovementWrite,
    };
    use crate::commands::settings::{PravnaForma, ShopProfile};
    use crate::db::{remove_test_database, test_database_path, Db};
    use crate::state::AppState;

    fn with_connection(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let mut connection = db.open().expect("database should open");
            seed_required_data(&mut connection);
            test(&mut connection);
        }

        remove_test_database(&path);
    }

    fn sign_in_admin(state: &AppState) -> i64 {
        state
            .set_session_user_id(SEEDED_ADMIN_ID)
            .expect("admin session should set");
        SEEDED_ADMIN_ID
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

    const SEEDED_ADMIN_ID: i64 = 1;

    /// Seeds a dobavljač and one isprava against it, returning the isprava id.
    fn seed_isprava(connection: &Connection, fizicko_lice: bool) -> i64 {
        let dobavljac_id = crate::dobavljaci::save_dobavljac(
            connection,
            crate::dobavljaci::SaveDobavljacRequest {
                id: None,
                poslovno_ime: if fizicko_lice {
                    "Petar Petrović".into()
                } else {
                    "ABC d.o.o.".into()
                },
                adresa: "Novi Pazar".into(),
                pib: "123456789".into(),
                maticni_broj_bpg: "20123456".into(),
                fizicko_lice,
            },
            "2026-06-18T10:00:00Z",
        )
        .expect("dobavljač should save");

        crate::dobavljaci::create_isprava(
            connection,
            crate::dobavljaci::CreateIspravaRequest {
                dobavljac_id,
                vrsta: "otpremnica".into(),
                broj: "125".into(),
                datum: "2026-07-03".into(),
                poseduje_ispravu: true,
                napomena: None,
            },
            SEEDED_ADMIN_ID,
            "2026-06-18T10:00:00Z",
        )
        .expect("isprava should create")
    }

    /// PEP čl. 15 st. 4 — kolona 3 names the document naziv, broj and datum plus
    /// the dobavljač. The composed line follows KEP-VERIFIED-RULES §2.7's worked
    /// ledger row, and the isprava's datum lands in `document_date`, which every
    /// receipt before this left NULL (PEP čl. 15 st. 3: the booking date and the
    /// document date are two different dates).
    #[test]
    fn a_receipt_with_an_isprava_composes_kolona3_and_dates_the_document() {
        with_connection("inventory_isprava_opis", |connection| {
            let isprava_id = seed_isprava(connection, false);
            let mut request = adjustment(5000, "Prijem robe");
            request.isprava_id = Some(isprava_id);

            apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                request,
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect("receive should succeed");

            let (opis, document_date): (String, Option<String>) = connection
                .query_row(
                    "SELECT opis, document_date FROM kep_entries
                     WHERE kolona = 'zaduzenje' ORDER BY id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("the zaduženje row");

            assert_eq!(
                opis, "Kalkulacija br. 1; otpremnica dobavljača „ABC d.o.o.“ br. 125 od 03.07.2026",
                "kolona 3 carries document naziv, broj, datum and the dobavljač"
            );
            assert_eq!(
                document_date.as_deref(),
                Some("2026-07-03"),
                "the DOCUMENT date, not the 04.07 booking date — never conflate them"
            );

            let isprava_on_kalkulacija: Option<i64> = connection
                .query_row(
                    "SELECT isprava_id FROM kalkulacije ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("the kalkulacija");
            assert_eq!(isprava_on_kalkulacija, Some(isprava_id));
        });
    }

    /// A natural-person supplier goes into kolona 3 as ime i prebivalište, and
    /// without the quotation marks that belong to a poslovno ime.
    #[test]
    fn a_natural_person_supplier_is_named_with_prebivaliste() {
        with_connection("inventory_isprava_fizicko", |connection| {
            let isprava_id = seed_isprava(connection, true);
            let mut request = adjustment(5000, "Prijem robe");
            request.isprava_id = Some(isprava_id);

            apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                request,
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect("receive should succeed");

            let opis: String = connection
                .query_row(
                    "SELECT opis FROM kep_entries WHERE kolona = 'zaduzenje' ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("the zaduženje row");
            assert_eq!(
                opis,
                "Kalkulacija br. 1; otpremnica dobavljača Petar Petrović, Novi Pazar br. 125 od 03.07.2026"
            );
        });
    }

    /// §3 req 12 and the čl. 34 precedent: warn, never block. All three writes
    /// must still be there, and the advisory must come back beside them.
    #[test]
    fn a_receipt_without_an_isprava_still_books_everything_and_warns() {
        with_connection("inventory_isprava_missing", |connection| {
            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect("a receipt with no isprava must NOT be refused");

            assert_eq!(result.new_quantity_milli, 5000, "the movement is written");

            let kalkulacije: i64 = connection
                .query_row("SELECT COUNT(*) FROM kalkulacije", [], |row| row.get(0))
                .expect("count");
            let zaduzenja: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM kep_entries WHERE kolona = 'zaduzenje'",
                    [],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(kalkulacije, 1, "the kalkulacija is written");
            assert_eq!(zaduzenja, 1, "the zaduženje is written");

            let warning = result
                .isprava_warning
                .expect("an unattached receipt must warn");
            assert!(
                warning.notice.citation.contains("čl. 29 st. 1"),
                "the advisory cites the duty it is about: {}",
                warning.notice.citation
            );
            // The opis stays the plain text — an unattached receipt reads as
            // exactly what it is rather than implying a document behind it.
            let opis: String = connection
                .query_row(
                    "SELECT opis FROM kep_entries WHERE kolona = 'zaduzenje' ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("the zaduženje row");
            assert_eq!(opis, "Prijem robe — Prijem robe");
        });
    }

    #[test]
    fn a_receipt_with_an_isprava_does_not_warn() {
        with_connection("inventory_isprava_no_warning", |connection| {
            let isprava_id = seed_isprava(connection, false);
            let mut request = adjustment(5000, "Prijem robe");
            request.isprava_id = Some(isprava_id);

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                request,
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect("receive should succeed");

            assert!(
                result.isprava_warning.is_none(),
                "nothing to warn about once the document is recorded"
            );
        });
    }

    /// An unknown isprava id must take the whole receipt down with it rather than
    /// booking goods against a document that does not exist.
    #[test]
    fn an_unknown_isprava_rolls_the_receipt_back() {
        with_connection("inventory_isprava_unknown", |connection| {
            let mut request = adjustment(5000, "Prijem robe");
            request.isprava_id = Some(999);

            let err = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                request,
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect_err("an unknown isprava is not bookable");
            assert_eq!(err.code(), "not_found");

            for (table, predicate) in [
                ("inventory_movements", ""),
                ("kalkulacije", ""),
                ("kep_entries", " WHERE kolona = 'zaduzenje'"),
            ] {
                let count: i64 = connection
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {table}{predicate}"),
                        [],
                        |row| row.get(0),
                    )
                    .expect("count");
                assert_eq!(count, 0, "{table} must roll back with the rest");
            }
        });
    }

    fn adjustment(quantity_milli: i64, reason: &str) -> InventoryAdjustmentRequest {
        InventoryAdjustmentRequest {
            product_id: 1,
            quantity_milli,
            reason: Some(reason.to_string()),
            purchase_price_minor: None,
            reference_type: None,
            reference_id: None,
            isprava_id: None,
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
                    SEEDED_ADMIN_ID,
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

    // SW-9b: a goods receipt generates exactly one kalkulacija (elements 1-14,
    // element 13 == qty × sale_price) atomically with the 9a zaduženje, and the
    // zaduženje links that kalkulacija as its isprava. Memo §3 worked example:
    // 50 kom @ 156,00 sa PDV -> element 13 = 7.800,00.
    #[test]
    fn receive_generates_kalkulacija_and_links_zaduzenje() {
        with_connection(
            "receive_generates_kalkulacija_and_links_zaduzenje",
            |connection| {
                connection
                    .execute(
                        "UPDATE products SET sale_price_minor = 15600 WHERE id = 1",
                        [],
                    )
                    .expect("price should update");

                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(50_000, "Prijem robe"),
                    SEEDED_ADMIN_ID,
                    "2026-07-04T09:00:00Z",
                )
                .expect("receive should succeed");

                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM kalkulacije", [], |row| row.get(0))
                    .expect("kalkulacija count should query");
                assert_eq!(count, 1, "exactly one kalkulacija per receipt");

                let (kalkulacija_id, sa_pdv): (i64, i64) = connection
                    .query_row(
                        "SELECT id, prodajna_vrednost_sa_pdv_minor FROM kalkulacije",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("kalkulacija row should query");
                assert_eq!(sa_pdv, 780_000, "element 13 = 50 × 156,00 sa PDV");

                let (reference_type, reference_id): (String, i64) = connection
                    .query_row(
                        "SELECT reference_type, reference_id FROM kep_entries WHERE kind = 'receipt'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("zaduzenje row should query");
                assert_eq!(reference_type, "kalkulacija", "zaduzenje links the isprava");
                assert_eq!(reference_id, kalkulacija_id);
            },
        );
    }

    // Corrections and write-offs are not receipts — they book no zaduženje (SW-9a)
    // and generate no kalkulacija (SW-9b).
    #[test]
    fn corrections_and_write_offs_generate_no_kalkulacija() {
        with_connection(
            "corrections_and_write_offs_generate_no_kalkulacija",
            |connection| {
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Correction,
                    adjustment(1000, "Popis"),
                    SEEDED_ADMIN_ID,
                    "2026-07-04T09:00:00Z",
                )
                .expect("correction should succeed");
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::WriteOff,
                    adjustment(500, "Otpis"),
                    SEEDED_ADMIN_ID,
                    "2026-07-04T09:00:00Z",
                )
                .expect("write-off should succeed");

                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM kalkulacije", [], |row| row.get(0))
                    .expect("kalkulacija count should query");
                assert_eq!(count, 0, "no kalkulacija for corrections/write-offs");
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
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed");

                let error = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::WriteOff,
                    adjustment(2000, "Lom"),
                    SEEDED_ADMIN_ID,
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
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed");
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Correction,
                    adjustment(-500, "Korekcija popisa"),
                    SEEDED_ADMIN_ID,
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
                SEEDED_ADMIN_ID,
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

    fn seed_rasprodaja(connection: &Connection, product_id: i64, status: &str) {
        connection
            .execute(
                "INSERT INTO campaigns (
                    id,
                    campaign_type,
                    status,
                    starts_on,
                    ends_on,
                    display_mode,
                    rasprodaja_ground,
                    separation_attested,
                    activated_at,
                    created_by,
                    created_at,
                    updated_at
                 )
                 VALUES (
                    1,
                    'rasprodaja',
                    ?1,
                    '2026-06-18',
                    NULL,
                    'two_prices',
                    'prestanak_poslovanja',
                    1,
                    '2026-06-18T09:00:00Z',
                    1,
                    '2026-06-18T09:00:00Z',
                    '2026-06-18T09:00:00Z'
                 )",
                params![status],
            )
            .expect("campaign should insert");

        connection
            .execute(
                "INSERT INTO campaign_items (
                    campaign_id,
                    product_id,
                    campaign_price_minor,
                    prethodna_cena_minor,
                    anchor_status,
                    anchor_window_days,
                    anchor_truncated,
                    pre_campaign_price_minor
                 )
                 VALUES (1, ?1, 9999, 12999, 'computed', 30, 0, 12999)",
                params![product_id],
            )
            .expect("campaign item should insert");
    }

    #[test]
    fn receive_is_blocked_while_article_is_in_an_active_rasprodaja() {
        with_connection(
            "receive_is_blocked_while_article_is_in_an_active_rasprodaja",
            |connection| {
                seed_rasprodaja(connection, 1, "active");

                let error = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(3000, "Prijem robe"),
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:00:00Z",
                )
                .expect_err("receive should be blocked during an active rasprodaja");
                assert_eq!(CommandError::from(error).code, "rasprodaja_receive_blocked");

                // The block writes nothing at all.
                let movement_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM inventory_movements WHERE product_id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("movement count should query");
                assert_eq!(movement_count, 0);

                // Counting and damage stay allowed: cl. 37 st. 7 bans adding new
                // quantities, not corrections or write-offs.
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Correction,
                    adjustment(500, "Korekcija popisa"),
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:30:00Z",
                )
                .expect("correction should stay allowed during a rasprodaja");
                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::WriteOff,
                    adjustment(200, "Lom"),
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:45:00Z",
                )
                .expect("write-off should stay allowed during a rasprodaja");

                // The block lifts when the rasprodaja ends.
                connection
                    .execute("UPDATE campaigns SET status = 'ended' WHERE id = 1", [])
                    .expect("campaign should end");

                let result = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(3000, "Prijem robe"),
                    SEEDED_ADMIN_ID,
                    "2026-06-19T12:00:00Z",
                )
                .expect("receive should succeed once the rasprodaja has ended");
                assert_eq!(result.new_quantity_milli, 3300);
            },
        );
    }

    #[test]
    fn receive_stays_open_for_articles_outside_the_rasprodaja() {
        with_connection(
            "receive_stays_open_for_articles_outside_the_rasprodaja",
            |connection| {
                connection
                    .execute(
                        "INSERT INTO products (
                            id, name, sku, category_id, unit_of_measure, sale_price_minor,
                            purchase_price_minor, tax_rate_id, minimum_stock_milli,
                            allow_negative_stock, created_at, updated_at
                         )
                         VALUES (
                            2, 'Jogurt 1 l', 'JOGURT-1L', 1, 'kom', 11999, 8500, 1, 0, 0,
                            '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z'
                         )",
                        [],
                    )
                    .expect("second product should insert");

                // Only product 1 is on rasprodaja.
                seed_rasprodaja(connection, 1, "active");

                let result = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    InventoryAdjustmentRequest {
                        product_id: 2,
                        quantity_milli: 4000,
                        reason: Some("Prijem robe".to_string()),
                        purchase_price_minor: None,
                        reference_type: None,
                        reference_id: None,
                        isprava_id: None,
                    },
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive of an unrelated article should succeed");
                assert_eq!(result.new_quantity_milli, 4000);
            },
        );
    }

    #[test]
    fn receive_is_allowed_while_a_draft_rasprodaja_is_unannounced() {
        with_connection(
            "receive_is_allowed_while_a_draft_rasprodaja_is_unannounced",
            |connection| {
                seed_rasprodaja(connection, 1, "draft");

                let result = apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(3000, "Prijem robe"),
                    SEEDED_ADMIN_ID,
                    "2026-06-18T12:00:00Z",
                )
                .expect("receive should succeed before the rasprodaja is announced");
                assert_eq!(result.new_quantity_milli, 3000);
            },
        );
    }

    #[test]
    fn correct_and_write_off_reject_cashier_but_receive_stays_open() {
        let path =
            test_database_path("correct_and_write_off_reject_cashier_but_receive_stays_open");

        {
            let db = Db::new(&path).expect("database should initialize");
            {
                let mut connection = db.open().expect("database should open");
                seed_required_data(&mut connection);
            }

            let state = AppState::new(db);
            sign_in_cashier(&state);

            // inventory_correct/inventory_write_off take a Tauri `State`, so a
            // headless mock app is needed to obtain a real managed state,
            // mirroring the admin-gate test pattern used in reports.rs.
            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let correction_error = inventory_correct(
                app.state::<AppState>(),
                adjustment(-500, "Korekcija popisa"),
            )
            .expect_err("cashier should not correct stock");
            assert_eq!(correction_error.code, "forbidden");

            let write_off_error =
                inventory_write_off(app.state::<AppState>(), adjustment(500, "Lom"))
                    .expect_err("cashier should not write off stock");
            assert_eq!(write_off_error.code, "forbidden");

            // Receiving stays open to cashiers.
            let receive_result =
                inventory_receive(app.state::<AppState>(), adjustment(1000, "Prijem robe"))
                    .expect("cashier should still be able to receive stock");
            assert_eq!(receive_result.new_quantity_milli, 1000);
        }

        remove_test_database(&path);
    }

    #[test]
    fn receiving_stock_posts_a_kep_zaduzenje_at_retail() {
        with_connection(
            "receiving_stock_posts_a_kep_zaduzenje_at_retail",
            |connection| {
                // Retail 156,00 incl. PDV — NOT the 100,00 nabavna. 50 kom -> 7.800,00.
                connection
                    .execute(
                        "UPDATE products
                         SET sale_price_minor = 15600, purchase_price_minor = 10000
                         WHERE id = 1",
                        [],
                    )
                    .expect("price update should apply");

                apply_inventory_adjustment(
                    connection,
                    InventoryMovementType::Receive,
                    adjustment(50_000, "Prijem robe"),
                    SEEDED_ADMIN_ID,
                    "2026-07-04T09:00:00Z",
                )
                .expect("receive should succeed");

                let (amount_minor, kolona, kind): (i64, String, String) = connection
                    .query_row(
                        "SELECT amount_minor, kolona, kind FROM kep_entries",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .expect("kep entry should query");

                assert_eq!(
                    amount_minor, 780000,
                    "50 x 156,00 retail incl. PDV — never the nabavna basis"
                );
                assert_eq!(kolona, "zaduzenje");
                assert_eq!(kind, "receipt");
            },
        );
    }

    #[test]
    fn a_correction_posts_no_kep_entry() {
        with_connection("a_correction_posts_no_kep_entry", |connection| {
            apply_inventory_adjustment(
                connection,
                InventoryMovementType::Correction,
                adjustment(500, "Korekcija popisa"),
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect("correction should succeed");

            let kep_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM kep_entries", [], |row| row.get(0))
                .expect("kep count should query");
            assert_eq!(kep_count, 0, "only goods receipts post a KEP zaduženje");
        });
    }

    #[test]
    fn receive_and_kep_entry_are_atomic() {
        with_connection("receive_and_kep_entry_are_atomic", |connection| {
            // A receive into a nonexistent product errors inside the transaction,
            // before the commit. Because the KEP zaduženje shares that transaction,
            // a rollback removes both — neither a movement nor a kep_entry can
            // persist without the other.
            let error = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                InventoryAdjustmentRequest {
                    product_id: 9999,
                    quantity_milli: 1000,
                    reason: Some("Prijem robe".to_string()),
                    purchase_price_minor: None,
                    reference_type: None,
                    reference_id: None,
                    isprava_id: None,
                },
                SEEDED_ADMIN_ID,
                "2026-07-04T09:00:00Z",
            )
            .expect_err("receive into a nonexistent product should fail");
            assert_eq!(CommandError::from(error).code, "not_found");

            let kep_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM kep_entries", [], |row| row.get(0))
                .expect("kep count should query");
            let movement_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM inventory_movements", [], |row| {
                    row.get(0)
                })
                .expect("movement count should query");
            assert_eq!(kep_count, 0, "the rolled-back zaduženje leaves nothing");
            assert_eq!(movement_count, 0, "the rolled-back movement leaves nothing");
        });
    }

    /// The seeded product carries no declaration identity data at all.
    fn seed_product_without_declaration(_connection: &Connection) -> i64 {
        1
    }

    fn seed_product_with_declaration(connection: &Connection) -> i64 {
        connection
            .execute(
                "UPDATE products
                 SET manufacturer_name = 'Mlekara Šabac d.o.o.',
                     country_of_origin = 'Srbija'
                 WHERE id = 1",
                [],
            )
            .expect("declaration data should update");
        1
    }

    /// Only the proizvođač is on file — the porijeklo column is still blank.
    fn seed_product_with_manufacturer_only(connection: &Connection) -> i64 {
        connection
            .execute(
                "UPDATE products
                 SET manufacturer_name = 'Mlekara Šabac d.o.o.'
                 WHERE id = 1",
                [],
            )
            .expect("declaration data should update");
        1
    }

    fn write_shop_profile_row(connection: &Connection, value_json: &str) {
        connection
            .execute(
                "INSERT INTO settings (key, value_json, updated_at)
                 VALUES ('shop_profile', ?1, '2026-07-31T10:00:00Z')
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![value_json],
            )
            .expect("shop profile row should write");
    }

    fn kep_entry_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM kep_entries", [], |row| row.get(0))
            .expect("kep count should query")
    }

    #[test]
    fn receiving_a_product_without_declaration_data_warns_but_never_blocks() {
        with_connection("receive_declaration_check_warning", |connection| {
            let product_id = seed_product_without_declaration(connection);

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5_000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-31T10:00:00Z",
            )
            .expect(
                "the receipt must succeed — the duty is the supplier's, \
                 and a block would strand stock",
            );

            assert_eq!(result.declaration_warnings.len(), 1);
            let warning = &result.declaration_warnings[0];
            assert_eq!(warning.product_id, product_id);
            assert!(
                warning.notice.is_legal_duty,
                "selling such goods IS an offence"
            );
            assert_eq!(
                warning.missing_fields,
                vec![
                    "manufacturerName".to_string(),
                    "countryOfOrigin".to_string()
                ],
                "the operator must be told which fields are blank"
            );

            // Pin the exact tier. `is_legal_duty` alone is true of every notice in
            // legal.rs, so it would not notice a swap to the čl. 67 defective tier
            // (fixed, lower, no trading ban) or to an unrelated statute. Compared
            // against the constructor rather than to literal text: no fine figure
            // may live outside legal.rs.
            assert_eq!(
                warning.notice,
                crate::legal::declaration_missing(&ShopProfile::default()),
                "the notice must be the čl. 68 st. 1 tač. 9 missing-declaration tier"
            );
            assert!(
                warning.notice.citation.contains("čl. 68 st. 1 tač. 9"),
                "the operator must be able to hand the inspector the right article: {}",
                warning.notice.citation
            );

            // Pravna forma is unanswered here, so no plausible figure may render.
            assert!(
                warning.notice.penalty.is_none(),
                "a shop of unknown legal form gets the duty with no figure: {:?}",
                warning.notice.penalty
            );

            // A blank catalog field is a record gap, not a proven offence: the
            // deklaracija lives on the packaging. The payload has to say so, or
            // the UI renders a shop-closure threat at a compliant shop.
            assert!(
                warning.advisory.contains("prekršaja nema"),
                "the record-vs-reality distinction must reach the operator: {}",
                warning.advisory
            );

            // Warn only: the goods are on the shelf and in the KEP either way.
            assert_eq!(result.new_quantity_milli, 5_000);
            assert_eq!(current_balance(connection), 5_000);
        });
    }

    #[test]
    fn a_declaration_warning_names_only_the_field_that_is_actually_blank() {
        with_connection("receive_declaration_check_partial", |connection| {
            seed_product_with_manufacturer_only(connection);

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5_000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-31T10:00:00Z",
            )
            .expect("receive should succeed");

            assert_eq!(result.declaration_warnings.len(), 1);
            assert_eq!(
                result.declaration_warnings[0].missing_fields,
                vec!["countryOfOrigin".to_string()],
                "the recorded proizvođač must not be reported as missing"
            );
        });
    }

    #[test]
    fn a_declaration_warning_renders_the_tier_of_the_shops_own_legal_form() {
        with_connection("receive_declaration_check_tier", |connection| {
            write_shop_profile_row(connection, r#"{"pravnaForma":"preduzetnik"}"#);

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5_000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-31T10:00:00Z",
            )
            .expect("receive should succeed");

            let warning = &result.declaration_warnings[0];
            let expected = crate::legal::declaration_missing(&ShopProfile {
                pravna_forma: Some(PravnaForma::Preduzetnik),
                ..ShopProfile::default()
            });
            assert_eq!(
                warning.notice, expected,
                "the preduzetnik must see the preduzetnik tier, resolved by legal.rs"
            );
            assert!(
                warning.notice.penalty.is_some(),
                "a known legal form does get a figure"
            );
        });
    }

    /// The advisory declaration check hangs off the receive transaction, after
    /// the KEP zaduženje. If it can raise, it rolls back the movement, the
    /// kalkulacija and the ledger entry — a hard block, which §3 req 12 forbids
    /// and which would strand a pallet already in the stockroom over a settings
    /// row the operator cannot even see from this screen. The catalog loader
    /// deliberately fails loud on a corrupt profile (that gate stays), so the
    /// advisory path has to absorb it.
    #[test]
    fn a_corrupt_shop_profile_cannot_block_a_goods_receipt() {
        with_connection("receive_declaration_check_corrupt_profile", |connection| {
            write_shop_profile_row(connection, "{ this is not json");

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5_000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-31T10:00:00Z",
            )
            .expect("an unreadable settings row must never strand received stock");

            assert_eq!(current_balance(connection), 5_000);
            assert_eq!(kep_entry_count(connection), 1, "the zaduženje still posts");

            let warning = &result.declaration_warnings[0];
            assert!(
                warning.notice.penalty.is_none(),
                "an unreadable profile resolves no tier, so no figure is invented: {:?}",
                warning.notice.penalty
            );
        });
    }

    #[test]
    fn receiving_a_product_with_declaration_data_raises_no_declaration_check_warning() {
        with_connection("receive_declaration_check_silent", |connection| {
            seed_product_with_declaration(connection);

            let result = apply_inventory_adjustment(
                connection,
                InventoryMovementType::Receive,
                adjustment(5_000, "Prijem robe"),
                SEEDED_ADMIN_ID,
                "2026-07-31T10:00:00Z",
            )
            .expect("receive should succeed");

            assert!(
                result.declaration_warnings.is_empty(),
                "a complete declaration must not nag: {:?}",
                result.declaration_warnings
            );
        });
    }

    #[test]
    fn marking_the_declaration_check_stamps_who_and_when() {
        let path = test_database_path("declaration_check_stamp");

        {
            let db = Db::new(&path).expect("database should initialize");
            {
                let mut connection = db.open().expect("database should open");
                seed_required_data(&mut connection);
                seed_product_with_declaration(&connection);
            }

            let state = AppState::new(db);
            let admin_id = sign_in_admin(&state);
            let product_id = 1;

            mark_declaration_checked(&state, product_id, "2026-07-31T10:00:00Z")
                .expect("the check should record");

            let conn = state.db().open().expect("db opens");
            let (at, by): (String, i64) = conn
                .query_row(
                    "SELECT declaration_checked_at, declaration_checked_by
                     FROM products WHERE id = ?1",
                    params![product_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("stamp should exist");

            assert_eq!(at, "2026-07-31T10:00:00Z");
            assert_eq!(by, admin_id);
        }

        remove_test_database(&path);
    }

    /// §3 req 25 puts the čl. 69a tač. 4 record at goods receipt because that is
    /// where the operator is standing — and `inventory_receive` admits the
    /// cashier. An admin-only stamp would be reachable only by someone who is
    /// not at the pallet, so the command must take the session, not the role.
    #[test]
    fn marking_the_declaration_check_is_open_to_the_cashier_who_receives_the_goods() {
        let path = test_database_path("declaration_check_stamp_cashier");

        {
            let db = Db::new(&path).expect("database should initialize");
            {
                let mut connection = db.open().expect("database should open");
                seed_required_data(&mut connection);
            }

            let state = AppState::new(db);
            sign_in_cashier(&state);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            inventory_mark_declaration_checked(app.state::<AppState>(), 1)
                .expect("the cashier at the pallet should be able to record the check");

            let conn = app
                .state::<AppState>()
                .db()
                .open()
                .expect("database should open");
            let (at, by): (String, i64) = conn
                .query_row(
                    "SELECT declaration_checked_at, declaration_checked_by
                     FROM products WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("stamp should exist");
            let cashier_id: i64 = conn
                .query_row("SELECT id FROM users WHERE username = 'marko'", [], |row| {
                    row.get(0)
                })
                .expect("cashier should exist");

            assert!(!at.is_empty(), "the stamp records when the operator looked");
            assert_eq!(
                by, cashier_id,
                "the stamp records the operator who looked, not an administrator"
            );
        }

        remove_test_database(&path);
    }
}
