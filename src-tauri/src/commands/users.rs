use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::cash_deposit::parse_iso_date;
use crate::clock::utc_now;
use crate::security::hash_credential;
use crate::state::AppState;

use super::auth::{require_admin, user_account_from_row, UserAccount};

/// ZoR čl. 51 st. 1 — „Puno radno vreme iznosi 40 časova nedeljno.“ Hours beyond
/// it are prekovremeni rad on a day's row, never a bigger contract figure.
const PUNO_RADNO_VREME_MINUTA_NEDELJNO: i64 = 40 * 60;

/// A šifra is a token from the Jedinstveni kodeks šifara, not a sentence. The
/// application holds no copy of the kodeks and cannot check membership in it —
/// this bound is only what separates a code from prose.
const MAX_SIFRA_DUZINA: usize = 16;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveUserRequest {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub pin: Option<String>,
    pub password: Option<String>,
    /// Absent means „this save does not carry the employee profile“, and the
    /// stored čl. 87–91 inputs are left exactly as they are. A role change or a
    /// PIN reset from any surface must never blank the guards `worktime.rs`
    /// reads.
    #[serde(default)]
    pub profile: Option<EmployeeProfile>,
}

/// The ZoR čl. 87–91 protection inputs plus the two ZEOR čl. 44 st. 2 codes, as
/// the Users screen edits them.
///
/// Two absences are deliberate and must stay that way:
///
/// 1. **`saglasnost_preraspodela_od` (čl. 57 st. 4) is not here.** It is a
///    legally distinct written consent from the čl. 91 one, and neither may ever
///    be read or written for the other's purpose — the same reason
///    `worktime::EmployeeProtection` leaves it out.
/// 2. **No free text.** `trudnoca_ili_dojenje` is a boolean and an „od“ date and
///    nothing else: the nalaz nadležnog zdravstvenog organa čl. 90 conditions the
///    prohibition on is never entered, attached or described here. A description
///    column on this struct would end up holding a diagnosis.
///
/// Neither consent date is a ZZPL pristanak, and neither field collects a
/// consent — each records that a written one exists and from when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployeeProfile {
    pub datum_rodjenja: Option<String>,
    pub datum_rodjenja_najmladjeg_deteta: Option<String>,
    pub samohrani_roditelj: Option<bool>,
    pub dete_tezak_invalid: Option<bool>,
    pub trudnoca_ili_dojenje: Option<bool>,
    pub trudnoca_ili_dojenje_od: Option<String>,
    pub radi_u_preraspodeli: bool,
    pub ugovoreno_radno_vreme_minuta_nedeljno: Option<i64>,
    pub zanimanje_sifra: Option<String>,
    pub kvalifikacija_sifra: Option<String>,
    pub saglasnost_prekovremeni_od: Option<String>,
}

#[tauri::command]
pub fn users_list(state: State<'_, AppState>) -> Result<Vec<UserAccount>, CommandError> {
    require_admin(state.inner())?;
    list_users(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn users_create(
    state: State<'_, AppState>,
    request: SaveUserRequest,
) -> Result<UserAccount, CommandError> {
    require_admin(state.inner())?;
    create_user(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn users_update(
    state: State<'_, AppState>,
    id: i64,
    request: SaveUserRequest,
) -> Result<UserAccount, CommandError> {
    require_admin(state.inner())?;
    update_user(state.inner(), id, request).map_err(Into::into)
}

#[tauri::command]
pub fn users_deactivate(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    require_admin(state.inner())?;
    deactivate_user(state.inner(), id).map_err(Into::into)
}

/// The employee profile is fetched one employee at a time and only by an admin.
///
/// It is not folded into `users_list`: `trudnoca_ili_dojenje` is health data, and
/// a list rendered on the Users screen would ship every employee's flag to the
/// client to draw one row. One employee, one deliberate action, one payload.
#[tauri::command]
pub fn users_employee_profile(
    state: State<'_, AppState>,
    id: i64,
) -> Result<EmployeeProfile, CommandError> {
    require_admin(state.inner())?;
    employee_profile(state.inner(), id).map_err(Into::into)
}

pub fn list_users(state: &AppState) -> Result<Vec<UserAccount>, AppError> {
    let conn = state.db().open()?;
    let mut stmt = conn.prepare(
        "SELECT id, username, display_name, role, active, created_at, updated_at, last_login_at
         FROM users
         ORDER BY active DESC, display_name ASC, username ASC",
    )?;
    let rows = stmt.query_map([], user_account_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_user(state: &AppState, request: SaveUserRequest) -> Result<UserAccount, AppError> {
    let normalized = normalize_request(request)?;

    if normalized.pin.is_none() && normalized.password.is_none() {
        return Err(AppError::validation(
            "Unesite PIN ili lozinku za novog korisnika.",
            serde_json::json!({ "field": "pin" }),
        ));
    }

    ensure_unique_username(state, &normalized.username, None)?;

    let now = utc_now()?;
    let pin_hash = normalized.pin.as_deref().map(hash_credential).transpose()?;
    let password_hash = normalized
        .password
        .as_deref()
        .map(hash_credential)
        .transpose()?;
    let active = i64::from(normalized.active);
    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO users (
            username,
            display_name,
            role,
            pin_hash,
            password_hash,
            active,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            normalized.username,
            normalized.display_name,
            normalized.role,
            pin_hash,
            password_hash,
            active,
            now
        ],
    )?;
    let user_id = tx.last_insert_rowid();
    if let Some(profile) = normalized.profile.as_ref() {
        write_profile(&tx, user_id, profile)?;
    }
    tx.commit()?;

    user_by_id(state, user_id)
}

pub fn update_user(
    state: &AppState,
    user_id: i64,
    request: SaveUserRequest,
) -> Result<UserAccount, AppError> {
    let normalized = normalize_request(request)?;
    ensure_unique_username(state, &normalized.username, Some(user_id))?;

    let existing = user_hashes(state, user_id)?
        .ok_or_else(|| AppError::not_found("Korisnik nije pronađen."))?;
    let pin_hash = match normalized.pin.as_deref() {
        Some(pin) => Some(hash_credential(pin)?),
        None => existing.pin_hash,
    };
    let password_hash = match normalized.password.as_deref() {
        Some(password) => Some(hash_credential(password)?),
        None => existing.password_hash,
    };

    if pin_hash.is_none() && password_hash.is_none() {
        return Err(AppError::validation(
            "Korisnik mora imati PIN ili lozinku.",
            serde_json::json!({ "field": "pin" }),
        ));
    }

    let now = utc_now()?;
    let active = i64::from(normalized.active);
    let mut conn = state.db().open()?;
    // The account and the čl. 87–91 profile move together: a half-applied save
    // would leave the guards reading one employee's inputs against another's
    // account state.
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE users
         SET username = ?1,
             display_name = ?2,
             role = ?3,
             active = ?4,
             pin_hash = ?5,
             password_hash = ?6,
             updated_at = ?7
         WHERE id = ?8",
        params![
            normalized.username,
            normalized.display_name,
            normalized.role,
            active,
            pin_hash,
            password_hash,
            now,
            user_id
        ],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Korisnik nije pronađen."));
    }

    if let Some(profile) = normalized.profile.as_ref() {
        write_profile(&tx, user_id, profile)?;
    }
    tx.commit()?;

    user_by_id(state, user_id)
}

/// Reads the čl. 87–91 profile of one employee.
///
/// `saglasnost_preraspodela_od` is not selected here for the same reason it is
/// absent from `EmployeeProfile`: the čl. 57 st. 4 consent is not the čl. 91 one,
/// and a surface that never receives it cannot render it as if it were.
pub fn employee_profile(state: &AppState, user_id: i64) -> Result<EmployeeProfile, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT datum_rodjenja,
                datum_rodjenja_najmladjeg_deteta,
                samohrani_roditelj,
                dete_tezak_invalid,
                trudnoca_ili_dojenje,
                trudnoca_ili_dojenje_od,
                radi_u_preraspodeli,
                ugovoreno_radno_vreme_minuta_nedeljno,
                zanimanje_sifra,
                kvalifikacija_sifra,
                saglasnost_prekovremeni_od
         FROM users
         WHERE id = ?1",
        params![user_id],
        |row| {
            Ok(EmployeeProfile {
                datum_rodjenja: row.get(0)?,
                datum_rodjenja_najmladjeg_deteta: row.get(1)?,
                samohrani_roditelj: row.get::<_, Option<i64>>(2)?.map(|flag| flag != 0),
                dete_tezak_invalid: row.get::<_, Option<i64>>(3)?.map(|flag| flag != 0),
                trudnoca_ili_dojenje: row.get::<_, Option<i64>>(4)?.map(|flag| flag != 0),
                trudnoca_ili_dojenje_od: row.get(5)?,
                radi_u_preraspodeli: row.get::<_, i64>(6)? != 0,
                ugovoreno_radno_vreme_minuta_nedeljno: row.get(7)?,
                zanimanje_sifra: row.get(8)?,
                kvalifikacija_sifra: row.get(9)?,
                saglasnost_prekovremeni_od: row.get(10)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Korisnik nije pronađen."))
}

/// Writes the profile columns and no others.
///
/// `saglasnost_preraspodela_od` is absent from this statement on purpose: the
/// čl. 57 st. 4 consent has no editor on this surface, so no save through the
/// Users screen can set, move or clear it.
fn write_profile(
    conn: &Connection,
    user_id: i64,
    profile: &EmployeeProfile,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE users
         SET datum_rodjenja = ?1,
             datum_rodjenja_najmladjeg_deteta = ?2,
             samohrani_roditelj = ?3,
             dete_tezak_invalid = ?4,
             trudnoca_ili_dojenje = ?5,
             trudnoca_ili_dojenje_od = ?6,
             radi_u_preraspodeli = ?7,
             ugovoreno_radno_vreme_minuta_nedeljno = ?8,
             zanimanje_sifra = ?9,
             kvalifikacija_sifra = ?10,
             saglasnost_prekovremeni_od = ?11
         WHERE id = ?12",
        params![
            profile.datum_rodjenja,
            profile.datum_rodjenja_najmladjeg_deteta,
            profile.samohrani_roditelj.map(i64::from),
            profile.dete_tezak_invalid.map(i64::from),
            profile.trudnoca_ili_dojenje.map(i64::from),
            profile.trudnoca_ili_dojenje_od,
            i64::from(profile.radi_u_preraspodeli),
            profile.ugovoreno_radno_vreme_minuta_nedeljno,
            profile.zanimanje_sifra,
            profile.kvalifikacija_sifra,
            profile.saglasnost_prekovremeni_od,
            user_id
        ],
    )?;

    Ok(())
}

pub fn deactivate_user(state: &AppState, user_id: i64) -> Result<(), AppError> {
    let now = utc_now()?;
    let conn = state.db().open()?;
    let changed = conn.execute(
        "UPDATE users SET active = 0, updated_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Korisnik nije pronađen."));
    }

    Ok(())
}

fn user_by_id(state: &AppState, user_id: i64) -> Result<UserAccount, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT id, username, display_name, role, active, created_at, updated_at, last_login_at
         FROM users
         WHERE id = ?1",
        params![user_id],
        user_account_from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Korisnik nije pronađen."))
}

fn ensure_unique_username(
    state: &AppState,
    username: &str,
    current_user_id: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.db().open()?;
    let duplicate_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE username = ?1 AND (?2 IS NULL OR id != ?2)",
        params![username, current_user_id],
        |row| row.get(0),
    )?;

    if duplicate_count > 0 {
        return Err(AppError::validation(
            "Korisničko ime već postoji.",
            serde_json::json!({ "field": "username" }),
        ));
    }

    Ok(())
}

struct UserHashes {
    pin_hash: Option<String>,
    password_hash: Option<String>,
}

fn user_hashes(state: &AppState, user_id: i64) -> Result<Option<UserHashes>, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT pin_hash, password_hash FROM users WHERE id = ?1",
        params![user_id],
        |row| {
            Ok(UserHashes {
                pin_hash: row.get(0)?,
                password_hash: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn normalize_request(request: SaveUserRequest) -> Result<SaveUserRequest, AppError> {
    let username = request.username.trim().to_string();
    let display_name = request.display_name.trim().to_string();
    let role = request.role.trim().to_string();

    if username.is_empty() {
        return Err(AppError::validation(
            "Korisničko ime je obavezno.",
            serde_json::json!({ "field": "username" }),
        ));
    }

    if display_name.is_empty() {
        return Err(AppError::validation(
            "Ime za prikaz je obavezno.",
            serde_json::json!({ "field": "displayName" }),
        ));
    }

    if role != "admin" && role != "cashier" {
        return Err(AppError::validation(
            "Uloga nije ispravna.",
            serde_json::json!({ "field": "role" }),
        ));
    }

    Ok(SaveUserRequest {
        username,
        display_name,
        role,
        active: request.active,
        pin: normalized_secret(request.pin),
        password: normalized_secret(request.password),
        profile: request.profile.map(normalize_profile).transpose()?,
    })
}

/// Validates the čl. 87–91 inputs before they reach the database.
///
/// v17's date CHECKs are GLOB shape tests, so `2009-13-45` passes them and is
/// stored — `worktime::check_protection` then reports the profile as unusable
/// instead of running the guard. That is the wrong place for the operator to
/// find out, so every date is read here with the same civil-date reader the
/// worktime module computes ages with.
fn normalize_profile(profile: EmployeeProfile) -> Result<EmployeeProfile, AppError> {
    let datum_rodjenja = normalized_date(
        profile.datum_rodjenja,
        "datumRodjenja",
        "Datum rođenja nije ispravan datum (GGGG-MM-DD). Na osnovu njega se primenjuju \
         ZoR čl. 87 i čl. 88 st. 1.",
    )?;
    let datum_rodjenja_najmladjeg_deteta = normalized_date(
        profile.datum_rodjenja_najmladjeg_deteta,
        "datumRodjenjaNajmladjegDeteta",
        "Datum rođenja najmlađeg deteta nije ispravan datum (GGGG-MM-DD). Na osnovu njega se \
         primenjuje ZoR čl. 91.",
    )?;
    let trudnoca_ili_dojenje_od = normalized_date(
        profile.trudnoca_ili_dojenje_od,
        "trudnocaIliDojenjeOd",
        "Datum nalaza nadležnog zdravstvenog organa nije ispravan datum (GGGG-MM-DD) \
         (ZoR čl. 90).",
    )?;
    let saglasnost_prekovremeni_od = normalized_date(
        profile.saglasnost_prekovremeni_od,
        "saglasnostPrekovremeniOd",
        "Datum pisane saglasnosti nije ispravan datum (GGGG-MM-DD). Saglasnost se evidentira \
         po ZoR čl. 91.",
    )?;

    // čl. 90 is a stored flag set from a nalaz nadležnog zdravstvenog organa, and
    // the only thing recorded about that nalaz is the date from which it holds.
    // Either both are present or neither is — a lone flag has no period, and a
    // lone date says nothing about what it refers to.
    if profile.trudnoca_ili_dojenje == Some(true) && trudnoca_ili_dojenje_od.is_none() {
        return Err(AppError::validation(
            "Uz evidentiranu trudnoću ili dojenje upišite datum od kog važi nalaz nadležnog \
             zdravstvenog organa (ZoR čl. 90). Sam nalaz se ne unosi niti prilaže.",
            serde_json::json!({ "field": "trudnocaIliDojenjeOd" }),
        ));
    }

    if trudnoca_ili_dojenje_od.is_some() && profile.trudnoca_ili_dojenje != Some(true) {
        return Err(AppError::validation(
            "Datum se evidentira samo uz evidentiranu trudnoću ili dojenje (ZoR čl. 90).",
            serde_json::json!({ "field": "trudnocaIliDojenje" }),
        ));
    }

    if let Some(minuta) = profile.ugovoreno_radno_vreme_minuta_nedeljno {
        if minuta <= 0 {
            return Err(AppError::validation(
                "Ugovoreno radno vreme mora biti veće od nule.",
                serde_json::json!({ "field": "ugovorenoRadnoVremeMinutaNedeljno" }),
            ));
        }

        if minuta > PUNO_RADNO_VREME_MINUTA_NEDELJNO {
            return Err(AppError::validation(
                "Ugovoreno radno vreme ne može biti duže od punog radnog vremena od 40 časova \
                 nedeljno (ZoR čl. 51 st. 1). Časovi preko toga su prekovremeni rad i unose se \
                 po danu.",
                serde_json::json!({ "field": "ugovorenoRadnoVremeMinutaNedeljno" }),
            ));
        }
    }

    Ok(EmployeeProfile {
        datum_rodjenja,
        datum_rodjenja_najmladjeg_deteta,
        samohrani_roditelj: profile.samohrani_roditelj,
        dete_tezak_invalid: profile.dete_tezak_invalid,
        trudnoca_ili_dojenje: profile.trudnoca_ili_dojenje,
        trudnoca_ili_dojenje_od,
        radi_u_preraspodeli: profile.radi_u_preraspodeli,
        ugovoreno_radno_vreme_minuta_nedeljno: profile.ugovoreno_radno_vreme_minuta_nedeljno,
        zanimanje_sifra: normalized_sifra(
            profile.zanimanje_sifra,
            "zanimanjeSifra",
            "Zanimanje se upisuje kao šifra iz Jedinstvenog kodeksa šifara, ne kao opis \
             (ZEOR čl. 44 st. 2).",
        )?,
        kvalifikacija_sifra: normalized_sifra(
            profile.kvalifikacija_sifra,
            "kvalifikacijaSifra",
            "Nivo i vrsta kvalifikacije upisuju se kao šifra iz Jedinstvenog kodeksa šifara, \
             ne kao opis (ZEOR čl. 44 st. 2).",
        )?,
        saglasnost_prekovremeni_od,
    })
}

fn normalized_date(
    value: Option<String>,
    field: &'static str,
    message: &str,
) -> Result<Option<String>, AppError> {
    let Some(datum) = normalized_secret(value) else {
        return Ok(None);
    };

    if parse_iso_date(&datum).is_none() {
        return Err(AppError::validation(
            message,
            serde_json::json!({ "field": field }),
        ));
    }

    Ok(Some(datum))
}

/// A šifra is a token: ASCII, no whitespace, bounded length. The kodeks itself is
/// not shipped with the application, so membership cannot be checked — what can
/// be refused is prose, which is what the ZEOR čl. 44 st. 2 duty is about.
fn normalized_sifra(
    value: Option<String>,
    field: &'static str,
    message: &str,
) -> Result<Option<String>, AppError> {
    let Some(sifra) = normalized_secret(value) else {
        return Ok(None);
    };

    let je_sifra = sifra.chars().count() <= MAX_SIFRA_DUZINA
        && sifra
            .chars()
            .all(|znak| znak.is_ascii_alphanumeric() || matches!(znak, '.' | '-' | '/'));

    if !je_sifra {
        return Err(AppError::validation(
            message,
            serde_json::json!({ "field": field }),
        ));
    }

    Ok(Some(sifra))
}

fn normalized_secret(value: Option<String>) -> Option<String> {
    value.and_then(|secret| {
        let trimmed = secret.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tauri::Manager;

    use super::{
        create_user, employee_profile, update_user, users_employee_profile, EmployeeProfile,
        SaveUserRequest,
    };
    use crate::app_error::AppError;
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

    fn save_request(username: &str) -> SaveUserRequest {
        SaveUserRequest {
            username: username.to_string(),
            display_name: "Jelena Đurić".to_string(),
            role: "cashier".to_string(),
            active: true,
            pin: Some("4321".to_string()),
            password: None,
            profile: None,
        }
    }

    /// Every čl. 87–91 input filled in, so a round-trip proves the whole set is
    /// persisted rather than the first column of the statement.
    fn full_profile() -> EmployeeProfile {
        EmployeeProfile {
            datum_rodjenja: Some("1994-04-11".to_string()),
            datum_rodjenja_najmladjeg_deteta: Some("2024-02-29".to_string()),
            samohrani_roditelj: Some(true),
            dete_tezak_invalid: Some(false),
            trudnoca_ili_dojenje: Some(true),
            trudnoca_ili_dojenje_od: Some("2026-05-04".to_string()),
            radi_u_preraspodeli: true,
            ugovoreno_radno_vreme_minuta_nedeljno: Some(2400),
            zanimanje_sifra: Some("5223.02".to_string()),
            kvalifikacija_sifra: Some("IV".to_string()),
            saglasnost_prekovremeni_od: Some("2026-06-01".to_string()),
        }
    }

    fn sign_in(state: &AppState, username: &str) {
        let user_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT id FROM users WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .expect("user should exist");
        state
            .set_session_user_id(user_id)
            .expect("session should set");
    }

    #[test]
    fn create_user_persists_every_cl_87_91_profile_field() {
        with_state("users_create_persists_profile", |state| {
            let mut request = save_request("jelena");
            request.profile = Some(full_profile());

            let created = create_user(state, request).expect("user should be created");
            let stored = employee_profile(state, created.id).expect("profile should load");

            assert_eq!(stored, full_profile());
        });
    }

    /// v17's CHECKs are GLOB shape tests, so `2009-13-45` reaches the database.
    /// `worktime::check_protection` then reports the profile as unusable instead
    /// of running the guard, which is the wrong place to find out.
    #[test]
    fn a_date_the_v17_glob_check_admits_is_still_rejected_here() {
        with_state("users_profile_rejects_non_civil_date", |state| {
            let mut request = save_request("nevena");
            request.profile = Some(EmployeeProfile {
                datum_rodjenja: Some("2009-13-45".to_string()),
                ..EmployeeProfile::default()
            });

            let error = create_user(state, request).expect_err("13th month is not a date");

            let AppError::Validation { message, details } = error else {
                panic!("expected a validation error")
            };
            assert!(
                message.contains("Datum rođenja") && message.contains("čl. 87"),
                "the message must name the field and the article it serves: {message}"
            );
            assert_eq!(
                details.expect("field details")["field"],
                serde_json::json!("datumRodjenja")
            );
        });
    }

    /// čl. 90 is „boolean + datum“ and nothing else: the nalaz nadležnog
    /// zdravstvenog organa is never entered, attached or described here.
    #[test]
    fn the_cl_90_flag_is_a_boolean_and_a_date_and_nothing_else() {
        let value =
            serde_json::to_value(EmployeeProfile::default()).expect("the profile should serialize");
        let mut keys: Vec<String> = value
            .as_object()
            .expect("the profile should serialize as an object")
            .keys()
            .cloned()
            .collect();
        keys.sort();

        let mut expected = vec![
            "datumRodjenja",
            "datumRodjenjaNajmladjegDeteta",
            "samohraniRoditelj",
            "deteTezakInvalid",
            "trudnocaIliDojenje",
            "trudnocaIliDojenjeOd",
            "radiUPreraspodeli",
            "ugovorenoRadnoVremeMinutaNedeljno",
            "zanimanjeSifra",
            "kvalifikacijaSifra",
            "saglasnostPrekovremeniOd",
        ];
        expected.sort_unstable();

        assert_eq!(
            keys, expected,
            "no field may be added to the employee profile without this list — a free-text \
             column here would end up holding a diagnosis or a doznaka number"
        );
    }

    #[test]
    fn the_cl_90_flag_and_its_date_are_saved_together_or_not_at_all() {
        with_state("users_profile_cl_90_pairing", |state| {
            let mut flag_without_date = save_request("milica");
            flag_without_date.profile = Some(EmployeeProfile {
                trudnoca_ili_dojenje: Some(true),
                trudnoca_ili_dojenje_od: None,
                ..EmployeeProfile::default()
            });

            let error =
                create_user(state, flag_without_date).expect_err("the flag needs its „od“ date");
            let AppError::Validation { message, .. } = error else {
                panic!("expected a validation error")
            };
            assert!(
                message.contains("čl. 90"),
                "the message must name čl. 90: {message}"
            );

            let mut date_without_flag = save_request("milica");
            date_without_flag.profile = Some(EmployeeProfile {
                trudnoca_ili_dojenje: None,
                trudnoca_ili_dojenje_od: Some("2026-05-04".to_string()),
                ..EmployeeProfile::default()
            });

            let error =
                create_user(state, date_without_flag).expect_err("a date alone says nothing");
            let AppError::Validation { message, .. } = error else {
                panic!("expected a validation error")
            };
            assert!(
                message.contains("čl. 90"),
                "the message must name čl. 90: {message}"
            );
        });
    }

    /// The two written consents are legally distinct. `saglasnost_preraspodela_od`
    /// (čl. 57 st. 4) is not on this surface at all, and no save may write it.
    #[test]
    fn saving_a_profile_never_writes_the_cl_57_st_4_consent() {
        with_state("users_profile_leaves_preraspodela_consent", |state| {
            let mut request = save_request("dragan");
            request.profile = Some(full_profile());
            let created = create_user(state, request).expect("user should be created");

            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "UPDATE users SET saglasnost_preraspodela_od = '2026-01-15' WHERE id = ?1",
                    params![created.id],
                )
                .expect("the čl. 57 st. 4 consent should seed");

            let mut update = save_request("dragan");
            update.profile = Some(EmployeeProfile {
                saglasnost_prekovremeni_od: Some("2026-07-01".to_string()),
                ..EmployeeProfile::default()
            });
            update_user(state, created.id, update).expect("user should update");

            let stored: Option<String> = connection
                .query_row(
                    "SELECT saglasnost_preraspodela_od FROM users WHERE id = ?1",
                    params![created.id],
                    |row| row.get(0),
                )
                .expect("the column should read");

            assert_eq!(
                stored.as_deref(),
                Some("2026-01-15"),
                "the čl. 91 consent and the čl. 57 st. 4 consent are not interchangeable"
            );
        });
    }

    /// A save from a surface that carries no profile — a role change, a
    /// deactivation, a PIN reset — must not blank the čl. 87–91 inputs the
    /// worktime guards read.
    #[test]
    fn an_update_without_a_profile_leaves_the_stored_profile_untouched() {
        with_state("users_update_keeps_profile", |state| {
            let mut request = save_request("branka");
            request.profile = Some(full_profile());
            let created = create_user(state, request).expect("user should be created");

            let mut role_change = save_request("branka");
            role_change.role = "admin".to_string();
            role_change.profile = None;
            update_user(state, created.id, role_change).expect("user should update");

            assert_eq!(
                employee_profile(state, created.id).expect("profile should load"),
                full_profile()
            );
        });
    }

    /// ZEOR čl. 44 st. 2 — zanimanje and kvalifikacija are codes from the
    /// Jedinstveni kodeks šifara. The application cannot check membership in the
    /// kodeks, but it can refuse prose.
    #[test]
    fn zanimanje_and_kvalifikacija_are_codes_not_descriptions() {
        with_state("users_profile_codes", |state| {
            let mut request = save_request("stevan");
            request.profile = Some(EmployeeProfile {
                zanimanje_sifra: Some("Trgovac u prodavnici odeće".to_string()),
                ..EmployeeProfile::default()
            });

            let error = create_user(state, request).expect_err("a description is not a code");
            let AppError::Validation { message, details } = error else {
                panic!("expected a validation error")
            };
            assert!(
                message.contains("šifra") && message.contains("ZEOR čl. 44 st. 2"),
                "the message must name the kodeks duty: {message}"
            );
            assert_eq!(
                details.expect("field details")["field"],
                serde_json::json!("zanimanjeSifra")
            );

            let mut kvalifikacija = save_request("stevan");
            kvalifikacija.profile = Some(EmployeeProfile {
                kvalifikacija_sifra: Some("srednja stručna sprema".to_string()),
                ..EmployeeProfile::default()
            });
            let error = create_user(state, kvalifikacija).expect_err("a description is not a code");
            let AppError::Validation { details, .. } = error else {
                panic!("expected a validation error")
            };
            assert_eq!(
                details.expect("field details")["field"],
                serde_json::json!("kvalifikacijaSifra")
            );

            let mut valid = save_request("stevan");
            valid.profile = Some(EmployeeProfile {
                zanimanje_sifra: Some("  5223.02  ".to_string()),
                kvalifikacija_sifra: Some("IV".to_string()),
                ..EmployeeProfile::default()
            });
            let created = create_user(state, valid).expect("a code should be accepted");

            let stored = employee_profile(state, created.id).expect("profile should load");
            assert_eq!(stored.zanimanje_sifra.as_deref(), Some("5223.02"));
            assert_eq!(stored.kvalifikacija_sifra.as_deref(), Some("IV"));
        });
    }

    /// ZoR čl. 51 st. 1 — puno radno vreme is 40 časova nedeljno. Hours beyond it
    /// are prekovremeni rad and belong on the day's row, never in the contract
    /// figure the čl. 53 checks are read against.
    #[test]
    fn contracted_weekly_time_stays_within_puno_radno_vreme() {
        with_state("users_profile_weekly_minutes", |state| {
            for minutes in [0_i64, -60, 2401] {
                let mut request = save_request("vesna");
                request.profile = Some(EmployeeProfile {
                    ugovoreno_radno_vreme_minuta_nedeljno: Some(minutes),
                    ..EmployeeProfile::default()
                });

                let error = create_user(state, request)
                    .err()
                    .unwrap_or_else(|| panic!("{minutes} minutes weekly must be rejected"));
                let AppError::Validation { details, .. } = error else {
                    panic!("expected a validation error for {minutes} minutes weekly")
                };
                assert_eq!(
                    details.expect("field details")["field"],
                    serde_json::json!("ugovorenoRadnoVremeMinutaNedeljno")
                );
            }

            let mut puno = save_request("vesna");
            puno.profile = Some(EmployeeProfile {
                ugovoreno_radno_vreme_minuta_nedeljno: Some(2400),
                ..EmployeeProfile::default()
            });
            let created = create_user(state, puno).expect("40 hours weekly is puno radno vreme");

            assert_eq!(
                employee_profile(state, created.id)
                    .expect("profile should load")
                    .ugovoreno_radno_vreme_minuta_nedeljno,
                Some(2400)
            );
        });
    }

    #[test]
    fn employee_profile_is_admin_only() {
        let path = test_database_path("users_profile_admin_only");

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);

            let mut request = save_request("kasirka");
            request.profile = Some(full_profile());
            let created = create_user(&state, request).expect("user should be created");

            sign_in(&state, "kasirka");

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = users_employee_profile(app.state::<AppState>(), created.id)
                .expect_err("a cashier must not read an employee profile");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
