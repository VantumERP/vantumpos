//! Envelope encryption for backup files (SW-2, ZZPL čl. 50 kriptozaštita).
//!
//! A random 32-byte *data key* encrypts the SQLite snapshot with
//! XChaCha20-Poly1305. The data key is itself wrapped with a key derived from
//! the admin passphrase via argon2id. Both the wrapped key and the argon2
//! salt travel inside every backup file, so a fresh machine + the passphrase
//! can always recover — while the live machine keeps the plain data key so
//! scheduled backups encrypt unattended.
//!
//! The public surface here is consumed by the backup command wiring (Task 7);
//! until that lands the functions have no in-crate caller outside tests.
#![allow(dead_code)]

use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};

use crate::app_error::AppError;

const MAGIC: &[u8; 5] = b"VPBK1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const WRAPPED_KEY_LEN: usize = 48; // 32-byte key + 16-byte AEAD tag
const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN + WRAPPED_KEY_LEN + NONCE_LEN;

pub struct BackupKeyMaterial {
    pub salt: Vec<u8>,
    pub wrap_nonce: Vec<u8>,
    pub wrapped_data_key: Vec<u8>,
    pub data_key: [u8; 32],
}

fn random_bytes(len: usize) -> Result<Vec<u8>, AppError> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).map_err(|error| {
        AppError::InvalidState(format!("Nasumični podaci nisu dostupni: {error}"))
    })?;
    Ok(buf)
}

fn wrapping_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], AppError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| {
            AppError::InvalidState(format!("Izvođenje ključa nije uspelo: {error}"))
        })?;
    Ok(key)
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(Key::from_slice(key))
}

pub fn derive_new_key_material(passphrase: &str) -> Result<BackupKeyMaterial, AppError> {
    let salt = random_bytes(SALT_LEN)?;
    let wrap_nonce = random_bytes(NONCE_LEN)?;
    let mut data_key = [0u8; 32];
    data_key.copy_from_slice(&random_bytes(32)?);

    let wrapping = wrapping_key(passphrase, &salt)?;
    let wrapped_data_key = cipher(&wrapping)
        .encrypt(XNonce::from_slice(&wrap_nonce), data_key.as_ref())
        .map_err(|_| AppError::InvalidState("Uvijanje ključa nije uspelo.".to_string()))?;

    Ok(BackupKeyMaterial {
        salt,
        wrap_nonce,
        wrapped_data_key,
        data_key,
    })
}

pub fn unwrap_data_key(
    passphrase: &str,
    salt: &[u8],
    wrap_nonce: &[u8],
    wrapped_data_key: &[u8],
) -> Result<[u8; 32], AppError> {
    let wrapping = wrapping_key(passphrase, salt)?;
    let key_bytes = cipher(&wrapping)
        .decrypt(XNonce::from_slice(wrap_nonce), wrapped_data_key)
        .map_err(|_| {
            AppError::validation(
                "Lozinka za šifrovanje nije ispravna.",
                serde_json::json!({ "field": "passphrase" }),
            )
        })?;
    let mut data_key = [0u8; 32];
    if key_bytes.len() != 32 {
        return Err(AppError::InvalidState(
            "Ključ šifrovanja je oštećen.".to_string(),
        ));
    }
    data_key.copy_from_slice(&key_bytes);
    Ok(data_key)
}

pub fn is_encrypted(file_bytes: &[u8]) -> bool {
    file_bytes.len() >= MAGIC.len() && &file_bytes[..MAGIC.len()] == MAGIC
}

pub fn encrypt_snapshot(plaintext: &[u8], km: &BackupKeyMaterial) -> Result<Vec<u8>, AppError> {
    let data_nonce = random_bytes(NONCE_LEN)?;
    let ciphertext = cipher(&km.data_key)
        .encrypt(XNonce::from_slice(&data_nonce), plaintext)
        .map_err(|_| {
            AppError::InvalidState("Šifrovanje rezervne kopije nije uspelo.".to_string())
        })?;

    if km.salt.len() != SALT_LEN
        || km.wrap_nonce.len() != NONCE_LEN
        || km.wrapped_data_key.len() != WRAPPED_KEY_LEN
    {
        return Err(AppError::InvalidState(
            "Materijal ključa je neispravan.".to_string(),
        ));
    }

    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&km.salt);
    out.extend_from_slice(&km.wrap_nonce);
    out.extend_from_slice(&km.wrapped_data_key);
    out.extend_from_slice(&data_nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_snapshot(
    file_bytes: &[u8],
    local_data_key: Option<&[u8; 32]>,
    passphrase: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    if !is_encrypted(file_bytes) || file_bytes.len() < HEADER_LEN {
        return Err(AppError::InvalidState(
            "Fajl nije šifrovana rezervna kopija.".to_string(),
        ));
    }

    let mut offset = MAGIC.len();
    let salt = &file_bytes[offset..offset + SALT_LEN];
    offset += SALT_LEN;
    let wrap_nonce = &file_bytes[offset..offset + NONCE_LEN];
    offset += NONCE_LEN;
    let wrapped_data_key = &file_bytes[offset..offset + WRAPPED_KEY_LEN];
    offset += WRAPPED_KEY_LEN;
    let data_nonce = &file_bytes[offset..offset + NONCE_LEN];
    offset += NONCE_LEN;
    let ciphertext = &file_bytes[offset..];

    // Prefer the local plain data key (same-machine restore); fall back to the
    // passphrase (fresh-machine disaster recovery).
    if let Some(key) = local_data_key {
        if let Ok(plaintext) = cipher(key).decrypt(XNonce::from_slice(data_nonce), ciphertext) {
            return Ok(plaintext);
        }
    }

    let passphrase = passphrase.ok_or_else(|| {
        AppError::validation(
            "Potrebna je lozinka za dešifrovanje rezervne kopije.",
            serde_json::json!({ "field": "passphrase" }),
        )
    })?;

    let data_key = unwrap_data_key(passphrase, salt, wrap_nonce, wrapped_data_key)?;
    cipher(&data_key)
        .decrypt(XNonce::from_slice(data_nonce), ciphertext)
        .map_err(|_| {
            AppError::validation(
                "Lozinka za šifrovanje nije ispravna.",
                serde_json::json!({ "field": "passphrase" }),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_local_key() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let plaintext = b"SQLite format 3\0 ... pretend db bytes".to_vec();
        let file = encrypt_snapshot(&plaintext, &km).expect("encrypt");
        assert!(is_encrypted(&file));
        let out = decrypt_snapshot(&file, Some(&km.data_key), None).expect("decrypt");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn round_trip_with_passphrase_on_fresh_machine() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let plaintext = b"db-bytes".to_vec();
        let file = encrypt_snapshot(&plaintext, &km).expect("encrypt");
        // No local key (fresh machine): decrypt via passphrase only.
        let out = decrypt_snapshot(&file, None, Some("tajna-lozinka")).expect("decrypt");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let file = encrypt_snapshot(b"db-bytes", &km).expect("encrypt");
        let err = decrypt_snapshot(&file, None, Some("pogrešna")).expect_err("must reject");
        assert_eq!(err.code(), "validation_error");
    }

    #[test]
    fn plaintext_is_not_detected_as_encrypted() {
        assert!(!is_encrypted(b"SQLite format 3\0"));
    }
}
