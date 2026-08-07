use rusqlite::{params, OptionalExtension, Row};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

pub(crate) const COMPANY_SETTINGS_KEY: &str = "company";
pub(crate) const RECEIPT_SETTINGS_KEY: &str = "receipt_numbering";
pub(crate) const BACKUP_SETTINGS_KEY: &str = "backup";
pub(crate) const BACKUP_ENCRYPTION_KEY: &str = "backup_encryption";
pub(crate) const SALES_SETTINGS_KEY: &str = "sales";
pub(crate) const SHOP_PROFILE_KEY: &str = "shop_profile";
pub(crate) const EUR_RATE_KEY: &str = "eur_rate";
pub(crate) const EUR_RATE_AUTO_REFRESH_KEY: &str = "eur_rate_auto_refresh";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanySettings {
    #[serde(default)]
    pub shop_name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub pib: String,
    #[serde(default)]
    pub registration_number: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub logo_path: Option<String>,
    #[serde(default = "default_currency")]
    pub currency: String,
}

impl Default for CompanySettings {
    fn default() -> Self {
        Self {
            shop_name: "VantumPOS".to_string(),
            address: String::new(),
            pib: String::new(),
            registration_number: String::new(),
            phone: String::new(),
            logo_path: None,
            currency: default_currency(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanySettingsRequest {
    pub shop_name: String,
    pub address: String,
    pub pib: String,
    pub registration_number: String,
    pub phone: String,
    pub logo_path: Option<String>,
    #[serde(default = "default_currency")]
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxRate {
    pub id: i64,
    pub name: String,
    pub rate_basis_points: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTaxRateRequest {
    pub id: Option<i64>,
    pub name: String,
    pub rate_basis_points: i64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptSettings {
    #[serde(default = "default_receipt_prefix")]
    pub prefix: String,
    #[serde(default = "default_next_sequence_number")]
    pub next_sequence_number: i64,
    #[serde(default = "default_reset_policy")]
    pub reset_policy: String,
}

impl Default for ReceiptSettings {
    fn default() -> Self {
        Self {
            prefix: default_receipt_prefix(),
            next_sequence_number: default_next_sequence_number(),
            reset_policy: default_reset_policy(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptSettingsRequest {
    pub prefix: String,
    pub next_sequence_number: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesSettings {
    #[serde(default)]
    pub allow_overselling: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesSettingsRequest {
    pub allow_overselling: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PravnaForma {
    Preduzetnik,
    PravnoLice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EsirTip {
    Esir,
    Lpfr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EsirElement {
    pub naziv: String,
    pub verzija: String,
    pub ib: String,
    pub tip: EsirTip,
    #[serde(default)]
    pub checked_on: Option<String>,
}

/// The shop's legal identity. `pravna_forma == None` is a real state meaning
/// "not yet answered" — it is NEVER inferred (e.g. from the PIB), because
/// guessing the tier is exactly what produced the penalty errors corrected in
/// commit 3d5aede. Legal copy renders no figure at all while it is None.
///
/// `distance_selling` is tri-state for the same reason. It is a `[LEGAL]` fact
/// question to the shop (§3 req 31, §5 Q-8): answering it "yes" flips the
/// manufacturer / importer / origin fields from `[PRUDENTIAL]` aids into a
/// statutory pre-purchase display duty (ZoT čl. 34 st. 5) and reverses §3
/// req 22. `false` is the LENIENT branch, so a plain `bool` would let silence
/// select it — the inference-from-silence this type exists to prevent. `None`
/// means "nije odgovoreno": the onboarding surface must ask, not assume.
///
/// `lpfr_carve_out_internet_only` and `lpfr_carve_out_own_used_assets` are the
/// two ZF čl. 6 st. 4 carve-outs (§3 req 32). They are tri-state for the same
/// reason: `true` is the LENIENT branch here — it excuses the shop from the
/// per-premises L-PFR floor — so silence must never be read as either answer.
/// An unanswered carve-out leaves the čl. 6 st. 4 duty standing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopProfile {
    #[serde(default)]
    pub pravna_forma: Option<PravnaForma>,
    #[serde(default)]
    pub pdv_obveznik: Option<bool>,
    #[serde(default)]
    pub distance_selling: Option<bool>,
    #[serde(default)]
    pub lpfr_in_premises: Option<bool>,
    #[serde(default)]
    pub lpfr_carve_out_internet_only: Option<bool>,
    #[serde(default)]
    pub lpfr_carve_out_own_used_assets: Option<bool>,
    #[serde(default)]
    pub esir_elements: Vec<EsirElement>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopProfileRequest {
    pub pravna_forma: Option<PravnaForma>,
    pub pdv_obveznik: Option<bool>,
    #[serde(default)]
    pub distance_selling: Option<bool>,
    pub lpfr_in_premises: Option<bool>,
    #[serde(default)]
    pub lpfr_carve_out_internet_only: Option<bool>,
    #[serde(default)]
    pub lpfr_carve_out_own_used_assets: Option<bool>,
    #[serde(default)]
    pub esir_elements: Vec<EsirElement>,
}

#[tauri::command]
pub fn settings_get_company(state: State<'_, AppState>) -> Result<CompanySettings, CommandError> {
    load_company_settings(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_update_company(
    state: State<'_, AppState>,
    request: CompanySettingsRequest,
) -> Result<CompanySettings, CommandError> {
    save_company_settings(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn settings_list_tax_rates(state: State<'_, AppState>) -> Result<Vec<TaxRate>, CommandError> {
    list_tax_rates(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_save_tax_rate(
    state: State<'_, AppState>,
    request: SaveTaxRateRequest,
) -> Result<TaxRate, CommandError> {
    save_tax_rate(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn settings_seed_tax_rates(
    state: State<'_, AppState>,
    in_vat_system: bool,
) -> Result<Vec<TaxRate>, CommandError> {
    seed_tax_rates(state.inner(), in_vat_system).map_err(Into::into)
}

#[tauri::command]
pub fn settings_get_receipt(state: State<'_, AppState>) -> Result<ReceiptSettings, CommandError> {
    load_receipt_settings(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_update_receipt(
    state: State<'_, AppState>,
    request: ReceiptSettingsRequest,
) -> Result<ReceiptSettings, CommandError> {
    save_receipt_settings(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn settings_get_sales(state: State<'_, AppState>) -> Result<SalesSettings, CommandError> {
    load_sales_settings(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_update_sales(
    state: State<'_, AppState>,
    request: SalesSettingsRequest,
) -> Result<SalesSettings, CommandError> {
    save_sales_settings(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn settings_get_shop_profile(state: State<'_, AppState>) -> Result<ShopProfile, CommandError> {
    load_shop_profile(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_update_shop_profile(
    state: State<'_, AppState>,
    request: ShopProfileRequest,
) -> Result<ShopProfile, CommandError> {
    save_shop_profile(state.inner(), request).map_err(Into::into)
}

/// Read-only and never admin-gated: the profile panel has to be able to say
/// what a missing L-PFR costs *this* shop before anything is saved.
#[tauri::command]
pub fn settings_lpfr_notice(
    state: State<'_, AppState>,
) -> Result<crate::legal::LegalNotice, CommandError> {
    lpfr_notice(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn settings_get_eur_rate(state: State<'_, AppState>) -> Result<EurRateStatus, CommandError> {
    let today = today_utc()?;
    eur_rate_status(state.inner(), &today).map_err(Into::into)
}

#[tauri::command]
pub fn settings_refresh_eur_rate(
    state: State<'_, AppState>,
) -> Result<EurRateStatus, CommandError> {
    let today = today_utc()?;
    refresh_eur_rate(state.inner(), &today).map_err(Into::into)
}

/// The saved status is recomputed against today rather than against the entered
/// `rateDate`: a manually entered rate carrying yesterday's date is stale, and
/// the surface must say so instead of reporting the day it was entered for.
#[tauri::command]
pub fn settings_set_manual_eur_rate(
    state: State<'_, AppState>,
    rate_minor: i64,
    rate_date: String,
) -> Result<EurRateStatus, CommandError> {
    set_manual_eur_rate(state.inner(), rate_minor, rate_date.trim())?;
    let today = today_utc()?;
    eur_rate_status(state.inner(), &today).map_err(Into::into)
}

/// Staleness is judged on the calendar day, so the RFC3339 stamp is cut to the
/// `YYYY-MM-DD` shape `rate_date` is stored in.
fn today_utc() -> Result<String, AppError> {
    let now = crate::clock::utc_now()?;
    Ok(now.split('T').next().unwrap_or(&now).to_string())
}

pub(crate) fn load_json_setting<T>(
    state: &AppState,
    key: &str,
    default_value: T,
) -> Result<T, AppError>
where
    T: DeserializeOwned,
{
    let conn = state.db().open()?;
    let stored: Option<String> = conn
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;

    match stored {
        Some(value) => serde_json::from_str(&value).map_err(|source| {
            AppError::InvalidState(format!("Podešavanja nisu ispravna: {source}"))
        }),
        None => Ok(default_value),
    }
}

pub(crate) fn save_json_setting<T>(state: &AppState, key: &str, value: &T) -> Result<(), AppError>
where
    T: Serialize,
{
    let value_json = serde_json::to_string(value)
        .map_err(|source| AppError::InvalidState(format!("Podešavanja nisu ispravna: {source}")))?;
    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO settings (key, value_json, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET
             value_json = excluded.value_json,
             updated_at = excluded.updated_at",
        params![key, value_json],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn load_company_settings(state: &AppState) -> Result<CompanySettings, AppError> {
    load_json_setting(state, COMPANY_SETTINGS_KEY, CompanySettings::default())
}

pub fn save_company_settings(
    state: &AppState,
    request: CompanySettingsRequest,
) -> Result<CompanySettings, AppError> {
    super::auth::require_admin(state)?;
    validate_company_request(&request)?;

    let settings = CompanySettings {
        shop_name: request.shop_name.trim().to_string(),
        address: request.address.trim().to_string(),
        pib: request.pib.trim().to_string(),
        registration_number: request.registration_number.trim().to_string(),
        phone: request.phone.trim().to_string(),
        logo_path: request.logo_path.and_then(|path| {
            let trimmed = path.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }),
        currency: request.currency.trim().to_uppercase(),
    };

    save_json_setting(state, COMPANY_SETTINGS_KEY, &settings)?;
    Ok(settings)
}

pub fn list_tax_rates(state: &AppState) -> Result<Vec<TaxRate>, AppError> {
    let conn = state.db().open()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, rate_basis_points, active
         FROM tax_rates
         ORDER BY rate_basis_points DESC, name ASC",
    )?;
    let rows = stmt.query_map([], tax_rate_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn save_tax_rate(state: &AppState, request: SaveTaxRateRequest) -> Result<TaxRate, AppError> {
    super::auth::require_admin(state)?;
    validate_tax_rate_request(&request)?;

    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;
    let active = i64::from(request.active);
    let name = request.name.trim();
    let tax_rate_id = if let Some(id) = request.id {
        let changed = tx.execute(
            "UPDATE tax_rates
             SET name = ?1,
                 rate_basis_points = ?2,
                 active = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4",
            params![name, request.rate_basis_points, active, id],
        )?;

        if changed == 0 {
            return Err(AppError::not_found("PDV stopa nije pronađena."));
        }

        id
    } else {
        tx.execute(
            "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'), datetime('now'))",
            params![name, request.rate_basis_points, active],
        )?;
        tx.last_insert_rowid()
    };

    let tax_rate = tx.query_row(
        "SELECT id, name, rate_basis_points, active FROM tax_rates WHERE id = ?1",
        params![tax_rate_id],
        tax_rate_from_row,
    )?;

    tx.commit()?;
    Ok(tax_rate)
}

pub fn seed_tax_rates(state: &AppState, in_vat_system: bool) -> Result<Vec<TaxRate>, AppError> {
    super::auth::require_admin(state)?;
    {
        let mut conn = state.db().open()?;
        let existing: i64 =
            conn.query_row("SELECT COUNT(*) FROM tax_rates", [], |row| row.get(0))?;
        if existing == 0 {
            let tx = conn.transaction()?;
            if in_vat_system {
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('PDV 20%', 2000, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('PDV 10%', 1000, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO tax_rates (name, rate_basis_points, active, created_at, updated_at)
                     VALUES ('Bez PDV-a', 0, 1, datetime('now'), datetime('now'))",
                    [],
                )?;
            }
            tx.commit()?;
        }
    }
    list_tax_rates(state)
}

pub fn load_receipt_settings(state: &AppState) -> Result<ReceiptSettings, AppError> {
    load_json_setting(state, RECEIPT_SETTINGS_KEY, ReceiptSettings::default())
}

pub fn save_receipt_settings(
    state: &AppState,
    request: ReceiptSettingsRequest,
) -> Result<ReceiptSettings, AppError> {
    super::auth::require_admin(state)?;
    validate_receipt_request(&request)?;

    let settings = ReceiptSettings {
        prefix: request.prefix.trim().to_string(),
        next_sequence_number: request.next_sequence_number,
        reset_policy: default_reset_policy(),
    };

    save_json_setting(state, RECEIPT_SETTINGS_KEY, &settings)?;
    Ok(settings)
}

pub fn load_sales_settings(state: &AppState) -> Result<SalesSettings, AppError> {
    load_json_setting(state, SALES_SETTINGS_KEY, SalesSettings::default())
}

pub fn save_sales_settings(
    state: &AppState,
    request: SalesSettingsRequest,
) -> Result<SalesSettings, AppError> {
    super::auth::require_admin(state)?;
    let settings = SalesSettings {
        allow_overselling: request.allow_overselling,
    };
    save_json_setting(state, SALES_SETTINGS_KEY, &settings)?;
    Ok(settings)
}

/// The cached EUR middle rate. Absent until the first refresh, so the AML
/// surface can tell "never fetched" from "fetched and stale".
pub fn load_eur_rate(state: &AppState) -> Result<Option<crate::nbs_rate::EurRate>, AppError> {
    load_json_setting(state, EUR_RATE_KEY, None)
}

pub fn save_eur_rate(state: &AppState, rate: &crate::nbs_rate::EurRate) -> Result<(), AppError> {
    save_json_setting(state, EUR_RATE_KEY, &Some(rate.clone()))
}

/// The cached rate plus the staleness verdict for the day it was asked about.
/// `checked_for` travels with the verdict so the surface can say *which* day the
/// rate was judged against instead of implying "now".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EurRateStatus {
    pub rate: Option<crate::nbs_rate::EurRate>,
    pub is_stale: bool,
    pub checked_for: String,
}

/// A rate is fresh only on its own date. An absent rate is stale — never
/// "fine": the AML čl. 46 st. 1 threshold is derived from it, so silence has to
/// read as "unknown", not as "no rate needed".
pub fn eur_rate_status(state: &AppState, today: &str) -> Result<EurRateStatus, AppError> {
    let rate = load_eur_rate(state)?;
    let is_stale = rate.as_ref().is_none_or(|r| r.rate_date != today);
    Ok(EurRateStatus {
        rate,
        is_stale,
        checked_for: today.to_string(),
    })
}

/// Refreshes from the NBS, degrading to the cached rate when the fetch fails.
/// A dead network must never surface as a command error here: the operator gets
/// the stale rate plus `is_stale`, and the sale is never blocked by the refresh
/// itself. The admin gate runs BEFORE the fetch, so a cashier cannot trigger
/// the outbound call at all.
pub fn refresh_eur_rate(state: &AppState, today: &str) -> Result<EurRateStatus, AppError> {
    refresh_eur_rate_with(state, today, crate::nbs_rate::fetch_nbs_middle_rate)
}

/// `refresh_eur_rate` with the fetch injected, so the degrade-never-block
/// behaviour above is testable without a network.
pub fn refresh_eur_rate_with(
    state: &AppState,
    today: &str,
    fetch: impl FnOnce() -> Result<crate::nbs_rate::EurRate, AppError>,
) -> Result<EurRateStatus, AppError> {
    super::auth::require_admin(state)?;

    match fetch() {
        Ok(rate) => {
            save_eur_rate(state, &rate)?;
        }
        Err(error) => {
            log::warn!("NBS rate refresh failed, keeping the cached rate: {error}");
        }
    }

    eur_rate_status(state, today)
}

/// The bookkeeping behind the once-a-day opportunistic refresh.
///
/// It records the last *attempt*, not the last success, and deliberately so: a
/// shop with no internet would otherwise pay the NBS timeout on every admin
/// login, all day, forever. One attempt a day is the whole budget.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EurRateAutoRefresh {
    /// `YYYY-MM-DD` of the last attempt, or `None` on a fresh install.
    #[serde(default)]
    pub last_attempted_on: Option<String>,
}

/// Is the signed-in user an admin? Unlike `require_admin` this answers `false`
/// instead of erroring, because the opportunistic refresh is a silent no-op for
/// a cashier — not a rejected action anybody should be told about.
fn session_user_is_admin(state: &AppState) -> Result<bool, AppError> {
    let Some(user_id) = state.session_user_id()? else {
        return Ok(false);
    };
    let Some(user) = super::auth::active_user_by_id(state, user_id)? else {
        return Ok(false);
    };
    Ok(user.role == "admin")
}

/// One opportunistic NBS refresh per calendar day, for admin sessions only.
///
/// Obtaining the rate is **not itself a statutory duty**: it is the precondition
/// for the AML čl. 46 st. 1 check the shop *is* bound by. On a fresh install
/// nothing has ever fetched it, so that check silently does not run — this is
/// what closes that gap without anyone having to go looking for the Kurs tab.
///
/// It is a best-effort side errand and nothing may hang off its result: the
/// caller runs it detached so it can never delay a login, a sale or a shift, and
/// a failure degrades into the existing path (cached rate + staleness warning,
/// or unavailable) exactly as a manual refresh does.
///
/// Returns whether an attempt was made, which is what the tests assert on.
pub fn auto_refresh_eur_rate_if_due(state: &AppState) -> Result<bool, AppError> {
    let today = today_utc()?;
    auto_refresh_eur_rate_if_due_with(state, &today, crate::nbs_rate::fetch_nbs_middle_rate)
}

/// `auto_refresh_eur_rate_if_due` with the day and the fetch injected, so the
/// once-a-day budget and the degrade path are testable without a network.
pub fn auto_refresh_eur_rate_if_due_with(
    state: &AppState,
    today: &str,
    fetch: impl FnOnce() -> Result<crate::nbs_rate::EurRate, AppError>,
) -> Result<bool, AppError> {
    // A cashier session (or none at all) never reaches the network, and never
    // spends the day's attempt either — otherwise the admin who signs in after
    // the morning cashier would find the budget already gone.
    if !session_user_is_admin(state)? {
        return Ok(false);
    }

    let mut marker: EurRateAutoRefresh = load_json_setting(
        state,
        EUR_RATE_AUTO_REFRESH_KEY,
        EurRateAutoRefresh::default(),
    )?;
    if marker.last_attempted_on.as_deref() == Some(today) {
        return Ok(false);
    }

    // Stamped *before* the fetch, on purpose: a failed attempt has to spend the
    // day just as a successful one does, or an offline shop retries on every
    // admin login and pays the timeout each time.
    marker.last_attempted_on = Some(today.to_string());
    save_json_setting(state, EUR_RATE_AUTO_REFRESH_KEY, &marker)?;

    match fetch() {
        Ok(rate) => save_eur_rate(state, &rate)?,
        Err(error) => {
            log::warn!("opportunistic NBS rate refresh failed, keeping the cached rate: {error}");
        }
    }

    Ok(true)
}

/// The plausible RSD/EUR band a hand-typed rate has to fall in, in para
/// (50–500 RSD/EUR). This is a **typo guard, not a legal figure**: no statute
/// names it, it is not a fine, and it must never move to `legal.rs`.
///
/// Its job is the extra digit. A rate entered ten times too high multiplies the
/// AML čl. 46 st. 1 dinar threshold (Task 7 computes `10_000 * rate_minor`) by
/// ten, so an already-unlawful cash amount passes with no warning and the sale
/// records the bogus rate as its own justification. Over-entry is the dangerous
/// direction; under-entry only over-warns.
const MANUAL_RATE_BAND_PARA: std::ops::RangeInclusive<i64> = 5_000..=50_000;

/// The manual fallback for the days the NBS is unreachable. Admin-only: this
/// value sets the dinar threshold the AML block is computed from.
///
/// `rate_date` clears the same bar the NBS path holds its own date to — it is
/// persisted as `aml_rate_date` on the sale and compared against today for
/// staleness, so a free-text date would jam the fallback permanently.
pub fn set_manual_eur_rate(
    state: &AppState,
    rate_minor: i64,
    rate_date: &str,
) -> Result<EurRateStatus, AppError> {
    super::auth::require_admin(state)?;
    if rate_minor <= 0 {
        return Err(AppError::validation(
            "Kurs mora biti veći od nule.",
            serde_json::json!({ "field": "rateMinor" }),
        ));
    }
    if !MANUAL_RATE_BAND_PARA.contains(&rate_minor) {
        return Err(AppError::validation(
            "Kurs mora biti između 50 i 500 dinara za 1 evro.",
            serde_json::json!({ "field": "rateMinor" }),
        ));
    }
    if !crate::nbs_rate::is_iso_date(rate_date) {
        return Err(AppError::validation(
            "Datum kursa mora biti u obliku GGGG-MM-DD.",
            serde_json::json!({ "field": "rateDate" }),
        ));
    }
    save_eur_rate(
        state,
        &crate::nbs_rate::EurRate {
            rate_minor,
            rate_date: rate_date.to_string(),
            source: crate::nbs_rate::RateSource::Manual,
        },
    )?;
    eur_rate_status(state, rate_date)
}

pub fn load_shop_profile(state: &AppState) -> Result<ShopProfile, AppError> {
    load_json_setting(state, SHOP_PROFILE_KEY, ShopProfile::default())
}

/// The ZF čl. 6 st. 4 notice, resolved against the **stored** legal form.
///
/// Only the tier is resolved here. Whether the duty is engaged at all depends on
/// the three live answers (L-PFR present, and the two carve-outs), which the
/// profile panel edits in place — so the gating belongs to the surface, while
/// the figure may come from nowhere but `legal.rs`.
pub fn lpfr_notice(state: &AppState) -> Result<crate::legal::LegalNotice, AppError> {
    Ok(crate::legal::lpfr_required(&load_shop_profile(state)?))
}

pub fn save_shop_profile(
    state: &AppState,
    request: ShopProfileRequest,
) -> Result<ShopProfile, AppError> {
    super::auth::require_admin(state)?;

    let mut elements = Vec::with_capacity(request.esir_elements.len());
    for element in request.esir_elements {
        let naziv = element.naziv.trim().to_string();
        let verzija = element.verzija.trim().to_string();
        let ib = element.ib.trim().to_string();
        if naziv.is_empty() || verzija.is_empty() || ib.is_empty() {
            return Err(AppError::validation(
                "Naziv, verzija i IB elementa su obavezni.",
                serde_json::json!({ "field": "esirElements" }),
            ));
        }
        elements.push(EsirElement {
            naziv,
            verzija,
            ib,
            tip: element.tip,
            checked_on: element.checked_on,
        });
    }

    let profile = ShopProfile {
        pravna_forma: request.pravna_forma,
        pdv_obveznik: request.pdv_obveznik,
        distance_selling: request.distance_selling,
        lpfr_in_premises: request.lpfr_in_premises,
        lpfr_carve_out_internet_only: request.lpfr_carve_out_internet_only,
        lpfr_carve_out_own_used_assets: request.lpfr_carve_out_own_used_assets,
        esir_elements: elements,
    };
    save_json_setting(state, SHOP_PROFILE_KEY, &profile)?;
    Ok(profile)
}

fn validate_company_request(request: &CompanySettingsRequest) -> Result<(), AppError> {
    if request.shop_name.trim().is_empty() {
        return Err(AppError::validation(
            "Naziv radnje je obavezan.",
            serde_json::json!({ "field": "shopName" }),
        ));
    }

    let pib = request.pib.trim();
    if !pib.is_empty() && (pib.len() != 9 || !pib.chars().all(|ch| ch.is_ascii_digit())) {
        return Err(AppError::validation(
            "PIB mora imati 9 cifara.",
            serde_json::json!({ "field": "pib" }),
        ));
    }

    if request.currency.trim().to_uppercase() != default_currency() {
        return Err(AppError::validation(
            "Valuta za lokalni MVP mora biti RSD.",
            serde_json::json!({ "field": "currency" }),
        ));
    }

    Ok(())
}

fn validate_tax_rate_request(request: &SaveTaxRateRequest) -> Result<(), AppError> {
    if request.name.trim().is_empty() {
        return Err(AppError::validation(
            "Naziv PDV stope je obavezan.",
            serde_json::json!({ "field": "name" }),
        ));
    }

    if !(0..=10_000).contains(&request.rate_basis_points) {
        return Err(AppError::validation(
            "PDV stopa mora biti između 0 i 100%.",
            serde_json::json!({ "field": "rateBasisPoints" }),
        ));
    }

    Ok(())
}

fn validate_receipt_request(request: &ReceiptSettingsRequest) -> Result<(), AppError> {
    if request.prefix.trim().is_empty() {
        return Err(AppError::validation(
            "Prefiks računa je obavezan.",
            serde_json::json!({ "field": "prefix" }),
        ));
    }

    if request.next_sequence_number <= 0 {
        return Err(AppError::validation(
            "Sledeći broj računa mora biti veći od nule.",
            serde_json::json!({ "field": "nextSequenceNumber" }),
        ));
    }

    Ok(())
}

fn tax_rate_from_row(row: &Row<'_>) -> rusqlite::Result<TaxRate> {
    let active: i64 = row.get(3)?;

    Ok(TaxRate {
        id: row.get(0)?,
        name: row.get(1)?,
        rate_basis_points: row.get(2)?,
        active: active == 1,
    })
}

fn default_currency() -> String {
    "RSD".to_string()
}

fn default_receipt_prefix() -> String {
    "VP-".to_string()
}

fn default_next_sequence_number() -> i64 {
    1
}

fn default_reset_policy() -> String {
    "none".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{remove_test_database, test_database_path, Db};
    use crate::nbs_rate::{EurRate, RateSource};
    use crate::state::AppState;

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        remove_test_database(&path);
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

    #[test]
    fn company_settings_round_trip_persists_json_value() {
        with_state("company_settings_round_trip_persists_json_value", |state| {
            sign_in_admin(state);

            let saved = save_company_settings(
                state,
                CompanySettingsRequest {
                    shop_name: "Vantum Market".to_string(),
                    address: "Bulevar 1, Beograd".to_string(),
                    pib: "123456789".to_string(),
                    registration_number: "87654321".to_string(),
                    phone: "+381 11 123 456".to_string(),
                    logo_path: Some("C:/logos/vantum.png".to_string()),
                    currency: "RSD".to_string(),
                },
            )
            .expect("company settings should save");

            assert_eq!(saved.shop_name, "Vantum Market");

            let loaded =
                load_company_settings(state).expect("company settings should load after save");

            assert_eq!(loaded, saved);
        });
    }

    #[test]
    fn company_settings_reject_invalid_pib() {
        with_state("company_settings_reject_invalid_pib", |state| {
            sign_in_admin(state);

            let error = save_company_settings(
                state,
                CompanySettingsRequest {
                    shop_name: "Vantum Market".to_string(),
                    address: "Bulevar 1, Beograd".to_string(),
                    pib: "12-345".to_string(),
                    registration_number: "87654321".to_string(),
                    phone: "+381 11 123 456".to_string(),
                    logo_path: None,
                    currency: "RSD".to_string(),
                },
            )
            .expect_err("invalid PIB should be rejected");

            let command_error: crate::app_error::CommandError = error.into();

            assert_eq!(command_error.code, "validation_error");
        });
    }

    #[test]
    fn company_settings_update_rejected_for_cashier() {
        with_state("company_settings_update_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let error = save_company_settings(
                state,
                CompanySettingsRequest {
                    shop_name: "Vantum Market".to_string(),
                    address: "Bulevar 1, Beograd".to_string(),
                    pib: "123456789".to_string(),
                    registration_number: "87654321".to_string(),
                    phone: "+381 11 123 456".to_string(),
                    logo_path: None,
                    currency: "RSD".to_string(),
                },
            )
            .expect_err("cashier should not update company settings");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn tax_rate_create_and_update_persists_active_state() {
        with_state(
            "tax_rate_create_and_update_persists_active_state",
            |state| {
                sign_in_admin(state);

                let created = save_tax_rate(
                    state,
                    SaveTaxRateRequest {
                        id: None,
                        name: "PDV 20%".to_string(),
                        rate_basis_points: 2000,
                        active: true,
                    },
                )
                .expect("tax rate should be created");

                let updated = save_tax_rate(
                    state,
                    SaveTaxRateRequest {
                        id: Some(created.id),
                        name: "PDV 20".to_string(),
                        rate_basis_points: 2000,
                        active: false,
                    },
                )
                .expect("tax rate should update");

                assert!(!updated.active);

                let rates = list_tax_rates(state).expect("tax rates should list");

                assert_eq!(rates, vec![updated]);
            },
        );
    }

    #[test]
    fn tax_rate_deactivate_keeps_row_in_list() {
        with_state("tax_rate_deactivate_keeps_row_in_list", |state| {
            sign_in_admin(state);

            let created = save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: None,
                    name: "PDV 20".to_string(),
                    rate_basis_points: 2000,
                    active: true,
                },
            )
            .expect("tax rate should be created");

            save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: Some(created.id),
                    name: created.name.clone(),
                    rate_basis_points: created.rate_basis_points,
                    active: false,
                },
            )
            .expect("tax rate should deactivate");

            let rates = list_tax_rates(state).expect("tax rates should list");
            let stored = rates
                .iter()
                .find(|rate| rate.id == created.id)
                .expect("deactivated rate should still be present");

            assert!(!stored.active);
        });
    }

    #[test]
    fn tax_rate_update_unknown_id_returns_not_found() {
        with_state("tax_rate_update_unknown_id_returns_not_found", |state| {
            sign_in_admin(state);

            let error = save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: Some(9_999),
                    name: "PDV 20".to_string(),
                    rate_basis_points: 2000,
                    active: false,
                },
            )
            .expect_err("updating a missing tax rate should fail");

            let command_error: crate::app_error::CommandError = error.into();

            assert_eq!(command_error.code, "not_found");
        });
    }

    #[test]
    fn tax_rate_save_rejected_for_cashier() {
        with_state("tax_rate_save_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let error = save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: None,
                    name: "PDV 20".to_string(),
                    rate_basis_points: 2000,
                    active: true,
                },
            )
            .expect_err("cashier should not save tax rates");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn receipt_numbering_round_trip_persists_no_reset_policy() {
        with_state(
            "receipt_numbering_round_trip_persists_no_reset_policy",
            |state| {
                sign_in_admin(state);

                let saved = save_receipt_settings(
                    state,
                    ReceiptSettingsRequest {
                        prefix: "VP-".to_string(),
                        next_sequence_number: 42,
                    },
                )
                .expect("receipt settings should save");

                assert_eq!(saved.reset_policy, "none");

                let loaded =
                    load_receipt_settings(state).expect("receipt settings should load after save");

                assert_eq!(loaded, saved);
            },
        );
    }

    #[test]
    fn receipt_numbering_update_allowed_for_admin() {
        with_state("receipt_numbering_update_allowed_for_admin", |state| {
            sign_in_admin(state);

            let saved = save_receipt_settings(
                state,
                ReceiptSettingsRequest {
                    prefix: "VP-".to_string(),
                    next_sequence_number: 7,
                },
            )
            .expect("admin should update receipt numbering");

            assert_eq!(saved.next_sequence_number, 7);
        });
    }

    #[test]
    fn receipt_numbering_update_rejected_for_cashier() {
        with_state("receipt_numbering_update_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let error = save_receipt_settings(
                state,
                ReceiptSettingsRequest {
                    prefix: "VP-".to_string(),
                    next_sequence_number: 7,
                },
            )
            .expect_err("cashier should not update receipt numbering");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn receipt_numbering_update_rejected_without_session() {
        with_state(
            "receipt_numbering_update_rejected_without_session",
            |state| {
                let error = save_receipt_settings(
                    state,
                    ReceiptSettingsRequest {
                        prefix: "VP-".to_string(),
                        next_sequence_number: 7,
                    },
                )
                .expect_err("anonymous caller should not update receipt numbering");

                assert_eq!(error.code(), "unauthorized");
            },
        );
    }

    #[test]
    fn seed_tax_rates_creates_vat_rates_when_in_system() {
        with_state("seed_tax_rates_vat", |state| {
            sign_in_admin(state);
            let rates = super::seed_tax_rates(state, true).expect("seed should succeed");
            assert_eq!(rates.len(), 2);
            let bps: Vec<i64> = rates.iter().map(|r| r.rate_basis_points).collect();
            assert!(bps.contains(&2000) && bps.contains(&1000), "got {bps:?}");
        });
    }

    #[test]
    fn seed_tax_rates_creates_single_zero_rate_when_not_in_system() {
        with_state("seed_tax_rates_novat", |state| {
            sign_in_admin(state);
            let rates = super::seed_tax_rates(state, false).expect("seed should succeed");
            assert_eq!(rates.len(), 1);
            assert_eq!(rates[0].rate_basis_points, 0);
        });
    }

    #[test]
    fn seed_tax_rates_is_idempotent() {
        with_state("seed_tax_rates_idempotent", |state| {
            sign_in_admin(state);
            super::seed_tax_rates(state, true).expect("first seed");
            let rates = super::seed_tax_rates(state, false).expect("second seed is a no-op");
            assert_eq!(rates.len(), 2, "existing rates must not be overwritten");
        });
    }

    #[test]
    fn seed_tax_rates_rejects_cashier() {
        with_state("seed_tax_rates_forbidden", |state| {
            sign_in_cashier(state);
            let error = super::seed_tax_rates(state, true).expect_err("cashier is forbidden");
            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn shop_profile_defaults_to_unset_and_never_infers_legal_form() {
        with_state("shop_profile_defaults_unset", |state| {
            let profile = load_shop_profile(state).expect("profile should load");

            assert_eq!(
                profile.pravna_forma, None,
                "legal form must never be inferred"
            );
            assert_eq!(profile.pdv_obveznik, None);
            assert_eq!(
                profile.distance_selling, None,
                "distance selling must read as UNANSWERED, never as an implicit 'no' \
                 that silently picks the lenient walk-in branch"
            );
            assert_eq!(profile.lpfr_in_premises, None);
            assert_eq!(
                profile.lpfr_carve_out_internet_only, None,
                "an unanswered carve-out must have an unanswered position — a \
                 default `false` would silently assert the duty applies, and a \
                 default `true` would silently excuse the shop from it"
            );
            assert_eq!(profile.lpfr_carve_out_own_used_assets, None);
            assert!(profile.esir_elements.is_empty());
        });
    }

    #[test]
    fn shop_profile_round_trips_and_requires_admin() {
        with_state("shop_profile_round_trip", |state| {
            let request = ShopProfileRequest {
                pravna_forma: Some(PravnaForma::Preduzetnik),
                pdv_obveznik: Some(false),
                distance_selling: Some(true),
                lpfr_in_premises: Some(false),
                lpfr_carve_out_internet_only: Some(true),
                lpfr_carve_out_own_used_assets: Some(false),
                esir_elements: vec![EsirElement {
                    naziv: "Master ESIR".to_string(),
                    verzija: "2.1.4".to_string(),
                    ib: "338".to_string(),
                    tip: EsirTip::Esir,
                    checked_on: Some("2026-07-31".to_string()),
                }],
            };

            let error = save_shop_profile(state, request.clone())
                .expect_err("a signed-out caller must be rejected");
            assert!(matches!(
                error,
                AppError::Business {
                    code: "unauthorized",
                    ..
                }
            ));

            sign_in_admin(state);
            let saved = save_shop_profile(state, request).expect("admin should save");

            assert_eq!(saved.pravna_forma, Some(PravnaForma::Preduzetnik));
            assert_eq!(saved.distance_selling, Some(true));
            assert_eq!(saved.lpfr_carve_out_internet_only, Some(true));
            assert_eq!(saved.lpfr_carve_out_own_used_assets, Some(false));
            assert_eq!(saved.esir_elements[0].verzija, "2.1.4");

            let reloaded = load_shop_profile(state).expect("profile should reload");
            assert_eq!(reloaded, saved);
        });
    }

    #[test]
    fn shop_profile_save_rejected_for_cashier() {
        with_state("shop_profile_forbidden_for_cashier", |state| {
            sign_in_cashier(state);

            let error = save_shop_profile(
                state,
                ShopProfileRequest {
                    pravna_forma: Some(PravnaForma::PravnoLice),
                    pdv_obveznik: Some(true),
                    distance_selling: Some(true),
                    lpfr_in_premises: Some(true),
                    lpfr_carve_out_internet_only: None,
                    lpfr_carve_out_own_used_assets: None,
                    esir_elements: Vec::new(),
                },
            )
            .expect_err("cashier is forbidden");

            assert_eq!(error.code(), "forbidden");
        });
    }

    /// The panel gates the ZF čl. 6 st. 4 notice on the live answers, but the
    /// **figure** may only ever come from `legal.rs`, resolved against the
    /// stored legal form. An UNSET form has to yield no figure at all rather
    /// than a plausible one.
    #[test]
    fn lpfr_notice_resolves_the_stored_tier_and_stays_silent_while_it_is_unset() {
        with_state("lpfr_notice_tier", |state| {
            sign_in_admin(state);

            let notice = lpfr_notice(state).expect("notice should resolve");
            assert!(
                notice.penalty.is_none(),
                "no legal form is stored yet, so no figure may be quoted: {:?}",
                notice.penalty
            );
            assert!(
                notice.citation.contains("čl. 6 st. 4"),
                "{}",
                notice.citation
            );

            save_shop_profile(
                state,
                ShopProfileRequest {
                    pravna_forma: Some(PravnaForma::Preduzetnik),
                    pdv_obveznik: Some(false),
                    distance_selling: Some(false),
                    lpfr_in_premises: Some(false),
                    lpfr_carve_out_internet_only: Some(false),
                    lpfr_carve_out_own_used_assets: Some(false),
                    esir_elements: Vec::new(),
                },
            )
            .expect("admin should save");

            let notice = lpfr_notice(state).expect("notice should resolve");
            let penalty = notice.penalty.expect("the stored tier is known");
            assert!(
                penalty.contains("50.000 do 500.000"),
                "the preduzetnik row is ZF čl. 15 st. 3: {penalty}"
            );
            assert!(
                !penalty.contains("2.000.000"),
                "the pravno-lice ceiling must never reach a preduzetnik: {penalty}"
            );
        });
    }

    #[test]
    fn eur_rate_cache_is_absent_until_saved_and_round_trips() {
        with_state("eur_rate_cache_round_trip", |state| {
            assert!(
                load_eur_rate(state).expect("rate should load").is_none(),
                "no rate is cached before the first refresh"
            );

            let rate = crate::nbs_rate::EurRate {
                rate_minor: 11723,
                rate_date: "2026-07-31".to_string(),
                source: crate::nbs_rate::RateSource::Nbs,
            };
            save_eur_rate(state, &rate).expect("rate should save");

            let reloaded = load_eur_rate(state)
                .expect("rate should reload")
                .expect("a saved rate is cached");
            assert_eq!(reloaded, rate, "para-per-EUR, date and source all survive");
        });
    }

    #[test]
    fn rate_status_reports_staleness_without_touching_the_network() {
        with_state("eur_rate_staleness", |state| {
            sign_in_admin(state);

            let status = eur_rate_status(state, "2026-07-31").expect("status should compute");
            assert!(status.rate.is_none(), "no rate cached yet");
            assert!(status.is_stale, "an absent rate is stale");

            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 11723,
                    rate_date: "2026-07-30".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate should save");

            let status = eur_rate_status(state, "2026-07-31").expect("status should compute");
            assert!(status.is_stale, "yesterday's rate is stale for today");
            assert_eq!(status.rate.expect("cached").rate_minor, 11723);

            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 11800,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Manual,
                },
            )
            .expect("rate should save");

            let status = eur_rate_status(state, "2026-07-31").expect("status should compute");
            assert!(
                !status.is_stale,
                "today's rate is fresh regardless of source"
            );
        });
    }

    #[test]
    fn manual_rate_rejects_a_non_positive_amount() {
        with_state("eur_rate_manual_validation", |state| {
            sign_in_admin(state);
            let error = set_manual_eur_rate(state, 0, "2026-07-31")
                .expect_err("a zero rate must be rejected");
            assert!(matches!(error, AppError::Validation { .. }));
        });
    }

    /// The manual fallback writes the same `rate_date` the NBS path does, and
    /// Task 8 persists it as `aml_rate_date` on the sale. A free-text date — most
    /// plausibly `31.07.2026`, the format NBS itself renders — would be stale
    /// forever (it can never equal today's ISO date), so the fallback could never
    /// clear the staleness warning it exists to clear.
    #[test]
    fn manual_rate_rejects_a_date_that_is_not_iso() {
        with_state("eur_rate_manual_date_shape", |state| {
            sign_in_admin(state);

            for bad_date in [
                "31.07.2026",
                "",
                "2026-7-31",
                "2026-13-01",
                "2026-07-32",
                "danas",
                "2026-07-31T00:00:00Z",
            ] {
                let result = set_manual_eur_rate(state, 11_723, bad_date);
                assert!(
                    matches!(&result, Err(AppError::Validation { .. })),
                    "'{bad_date}' must be rejected as a validation error, got {result:?}"
                );
            }

            assert!(
                load_eur_rate(state).expect("rate should load").is_none(),
                "a rejected date must not have written a rate"
            );
        });
    }

    /// A fat-fingered extra digit (117_230 instead of 11_723) multiplies the AML
    /// čl. 46 st. 1 dinar threshold by ten, so an already-unlawful cash amount
    /// passes with no warning — and the sale records the bogus rate as its own
    /// justification. The band is a typo guard, not a legal figure.
    #[test]
    fn manual_rate_rejects_an_implausible_magnitude() {
        with_state("eur_rate_manual_band", |state| {
            sign_in_admin(state);

            let error = set_manual_eur_rate(state, 117_230, "2026-07-31")
                .expect_err("1.172,30 RSD/EUR is a typo, not a rate");
            assert!(matches!(error, AppError::Validation { .. }));
            assert!(
                load_eur_rate(state).expect("rate should load").is_none(),
                "an out-of-band rate must not be written"
            );

            let error = set_manual_eur_rate(state, 1_172, "2026-07-31")
                .expect_err("11,72 RSD/EUR is a typo, not a rate");
            assert!(matches!(error, AppError::Validation { .. }));

            set_manual_eur_rate(state, 11_723, "2026-07-31").expect("an in-band rate saves");
            assert_eq!(
                load_eur_rate(state)
                    .expect("rate should load")
                    .expect("the in-band rate is cached")
                    .rate_minor,
                11_723
            );
        });
    }

    /// An unreachable NBS may only leave the cached rate standing and stale — it
    /// must never surface as a command error, because that would block the
    /// surface the AML check hangs off (VERIFIED-RULES §3 SW-11(a) item 3).
    #[test]
    fn a_failed_fetch_degrades_to_the_cached_rate_instead_of_erroring() {
        with_state("eur_rate_refresh_degrades", |state| {
            sign_in_admin(state);
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 11_723,
                    rate_date: "2026-07-30".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate should save");

            let status = refresh_eur_rate_with(state, "2026-07-31", || {
                Err(AppError::business(
                    "nbs_rate_unavailable",
                    "Kurs NBS-a trenutno nije dostupan.",
                ))
            })
            .expect("a dead network must degrade, never block");

            let rate = status.rate.expect("the cached rate still stands");
            assert_eq!(rate.rate_minor, 11_723);
            assert_eq!(rate.rate_date, "2026-07-30");
            assert!(
                status.is_stale,
                "the cached rate is yesterday's, so the operator must be warned"
            );
        });
    }

    #[test]
    fn a_successful_fetch_replaces_the_cached_rate() {
        with_state("eur_rate_refresh_saves", |state| {
            sign_in_admin(state);

            let status = refresh_eur_rate_with(state, "2026-07-31", || {
                Ok(EurRate {
                    rate_minor: 11_800,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                })
            })
            .expect("a fetched rate should save");

            assert!(!status.is_stale, "today's rate is fresh");
            let cached = load_eur_rate(state)
                .expect("rate should load")
                .expect("the fetched rate is cached");
            assert_eq!(cached.rate_minor, 11_800);
            assert_eq!(cached.source, RateSource::Nbs);
        });
    }

    /// A fresh install has no cached rate at all, so the AML čl. 46 st. 1 check
    /// silently does not run until an admin happens to open Podešavanja → Kurs.
    /// The opportunistic refresh closes that gap — but at a cost of at most one
    /// attempt a day, because a shop with no internet would otherwise pay an
    /// 8-second NBS timeout on every admin login.
    #[test]
    fn the_auto_refresh_attempts_the_rate_at_most_once_a_day() {
        with_state("eur_rate_auto_refresh_once_a_day", |state| {
            sign_in_admin(state);

            let calls = std::cell::Cell::new(0_u32);
            let fetch = |rate_date: &str| {
                calls.set(calls.get() + 1);
                Ok(EurRate {
                    rate_minor: 11_723,
                    rate_date: rate_date.to_string(),
                    source: RateSource::Nbs,
                })
            };

            assert!(
                auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || fetch("2026-07-31"))
                    .expect("the first attempt of the day should run"),
                "a fresh install has never attempted a fetch"
            );
            assert_eq!(calls.get(), 1);

            assert!(
                !auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || fetch("2026-07-31"))
                    .expect("a second same-day call should be a no-op"),
                "the day's attempt is already spent"
            );
            assert_eq!(calls.get(), 1, "no second outbound call on the same day");

            let attempted_next_day =
                auto_refresh_eur_rate_if_due_with(state, "2026-08-01", || fetch("2026-08-01"))
                    .expect("the next day earns a fresh attempt");
            assert!(attempted_next_day, "a new calendar day refills the budget");
            assert_eq!(calls.get(), 2);
            assert_eq!(
                load_eur_rate(state)
                    .expect("rate should load")
                    .expect("the fetched rate is cached")
                    .rate_date,
                "2026-08-01"
            );
        });
    }

    /// A failed opportunistic refresh must leave the app exactly as it was:
    /// no error out of the call, the cached rate untouched, and the day's
    /// attempt spent so a dead network is not retried on every admin action.
    #[test]
    fn a_failed_auto_refresh_leaves_the_cached_rate_and_the_app_usable() {
        with_state("eur_rate_auto_refresh_degrades", |state| {
            sign_in_admin(state);
            save_eur_rate(
                state,
                &EurRate {
                    rate_minor: 11_723,
                    rate_date: "2026-07-30".to_string(),
                    source: RateSource::Nbs,
                },
            )
            .expect("rate should save");

            let attempted = auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || {
                Err(AppError::business(
                    "nbs_rate_unavailable",
                    "Kurs NBS-a trenutno nije dostupan.",
                ))
            })
            .expect("a dead network must never surface as an error here");
            assert!(attempted, "the attempt was made and is now spent");

            let status = eur_rate_status(state, "2026-07-31").expect("status should compute");
            let rate = status.rate.expect("the cached rate still stands");
            assert_eq!(rate.rate_minor, 11_723);
            assert_eq!(rate.rate_date, "2026-07-30");
            assert!(status.is_stale, "the operator is warned, not blocked");

            let calls = std::cell::Cell::new(0_u32);
            assert!(
                !auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || {
                    calls.set(calls.get() + 1);
                    Err(AppError::business("nbs_rate_unavailable", "i dalje ništa."))
                })
                .expect("the same-day retry is a no-op"),
                "a failed attempt still spends the day, so an offline shop stops retrying"
            );
            assert_eq!(calls.get(), 0);
        });
    }

    /// The opportunistic refresh is an admin-session affordance. A cashier
    /// session must not reach the network at all — and must not spend the day's
    /// attempt either, or the admin who signs in later would find it gone.
    #[test]
    fn the_auto_refresh_never_runs_for_a_cashier_session() {
        with_state("eur_rate_auto_refresh_cashier", |state| {
            sign_in_cashier(state);

            let calls = std::cell::Cell::new(0_u32);
            assert!(
                !auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || {
                    calls.set(calls.get() + 1);
                    Ok(EurRate {
                        rate_minor: 11_723,
                        rate_date: "2026-07-31".to_string(),
                        source: RateSource::Nbs,
                    })
                })
                .expect("a cashier session is a silent no-op, not an error"),
                "a cashier must never trigger the outbound NBS call"
            );
            assert_eq!(calls.get(), 0);
            assert!(
                load_eur_rate(state).expect("rate should load").is_none(),
                "nothing was fetched, so nothing was cached"
            );

            sign_in_admin(state);
            assert!(
                auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || {
                    calls.set(calls.get() + 1);
                    Ok(EurRate {
                        rate_minor: 11_723,
                        rate_date: "2026-07-31".to_string(),
                        source: RateSource::Nbs,
                    })
                })
                .expect("the admin session should attempt"),
                "the cashier session must not have consumed the day's attempt"
            );
            assert_eq!(calls.get(), 1);
        });
    }

    /// No session at all (the app is open at the login screen) must not fetch
    /// either — the refresh belongs to an admin session, not to app launch.
    #[test]
    fn the_auto_refresh_never_runs_without_a_session() {
        with_state("eur_rate_auto_refresh_no_session", |state| {
            let calls = std::cell::Cell::new(0_u32);
            let attempted = auto_refresh_eur_rate_if_due_with(state, "2026-07-31", || {
                calls.set(calls.get() + 1);
                Ok(EurRate {
                    rate_minor: 11_723,
                    rate_date: "2026-07-31".to_string(),
                    source: RateSource::Nbs,
                })
            })
            .expect("no session is a silent no-op");

            assert!(!attempted, "app launch alone must not reach the NBS");
            assert_eq!(calls.get(), 0);
        });
    }

    /// The manual rate sets the AML čl. 46 st. 1 threshold the block is derived
    /// from, so it is admin-only — and the gate has to fire before the refresh
    /// reaches the network, otherwise a cashier could still trigger the call.
    #[test]
    fn manual_and_refresh_rates_are_admin_only() {
        with_state("eur_rate_admin_gate", |state| {
            sign_in_cashier(state);

            let error = set_manual_eur_rate(state, 11_723, "2026-07-31")
                .expect_err("cashier must not set the AML rate");
            assert_eq!(error.code(), "forbidden");

            let error =
                refresh_eur_rate(state, "2026-07-31").expect_err("cashier must not refresh");
            assert_eq!(
                error.code(),
                "forbidden",
                "the admin gate must precede the network call"
            );

            assert!(
                load_eur_rate(state).expect("rate should load").is_none(),
                "a rejected call must not have written a rate"
            );
        });
    }
}
