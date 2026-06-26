use rusqlite::{params, OptionalExtension, Row};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

pub(crate) const COMPANY_SETTINGS_KEY: &str = "company";
pub(crate) const RECEIPT_SETTINGS_KEY: &str = "receipt_numbering";
pub(crate) const BACKUP_SETTINGS_KEY: &str = "backup";

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
            AppError::InvalidState(format!("Podesavanja nisu ispravna: {source}"))
        }),
        None => Ok(default_value),
    }
}

pub(crate) fn save_json_setting<T>(state: &AppState, key: &str, value: &T) -> Result<(), AppError>
where
    T: Serialize,
{
    let value_json = serde_json::to_string(value)
        .map_err(|source| AppError::InvalidState(format!("Podesavanja nisu ispravna: {source}")))?;
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
            return Err(AppError::not_found("PDV stopa nije pronadjena."));
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
            "PDV stopa mora biti izmedju 0 i 100%.",
            serde_json::json!({ "field": "rateBasisPoints" }),
        ));
    }

    Ok(())
}

fn validate_receipt_request(request: &ReceiptSettingsRequest) -> Result<(), AppError> {
    if request.prefix.trim().is_empty() {
        return Err(AppError::validation(
            "Prefiks racuna je obavezan.",
            serde_json::json!({ "field": "prefix" }),
        ));
    }

    if request.next_sequence_number <= 0 {
        return Err(AppError::validation(
            "Sledeci broj racuna mora biti veci od nule.",
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
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

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
    fn tax_rate_create_and_update_persists_active_state() {
        with_state(
            "tax_rate_create_and_update_persists_active_state",
            |state| {
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
}
