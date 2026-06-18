use argon2::Argon2;
use password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

use crate::app_error::AppError;

pub fn hash_credential(credential: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);

    Argon2::default()
        .hash_password(credential.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|source| AppError::InvalidState(format!("Credential hash nije sacuvan: {source}")))
}

pub fn verify_credential(credential: &str, encoded_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(encoded_hash) else {
        return false;
    };

    Argon2::default()
        .verify_password(credential.as_bytes(), &parsed_hash)
        .is_ok()
}
