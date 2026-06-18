use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app_error::AppError;

pub fn utc_now() -> Result<String, AppError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))
}
