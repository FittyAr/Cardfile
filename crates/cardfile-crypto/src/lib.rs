//! Password hashing and card encryption.
//!
//! - Account passwords are stored as Argon2id PHC strings.
//! - Encrypted cards use a key derived from a password with Argon2id (per-card random
//!   salt) and XChaCha20-Poly1305. The card id is bound as associated data so a
//!   ciphertext cannot be moved to another card.

use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};
use cardfile_core::EncryptedPayload;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use thiserror::Error;
use zeroize::Zeroizing;

/// Envelope format version written to [`EncryptedPayload::v`].
pub const PAYLOAD_VERSION: u32 = 1;

/// Argon2id cost used for new hashes and keys: 64 MiB, 3 passes, 1 lane.
const M_COST: u32 = 64 * 1024;
const T_COST: u32 = 3;
const P_COST: u32 = 1;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("wrong password or corrupted data")]
    Decrypt,
    #[error("unsupported encryption format version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid encrypted payload: {0}")]
    Malformed(String),
    #[error("system random number generator failed")]
    Random,
    #[error("key derivation failed: {0}")]
    Kdf(String),
}

pub type CryptoResult<T> = Result<T, CryptoError>;

fn argon2(m: u32, t: u32, p: u32) -> CryptoResult<Argon2<'static>> {
    let params =
        Params::new(m, t, p, Some(KEY_LEN)).map_err(|e| CryptoError::Kdf(e.to_string()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Hash an account password into an Argon2id PHC string.
pub fn hash_password(password: &str) -> CryptoResult<String> {
    let hash = argon2(M_COST, T_COST, P_COST)?
        .hash_password(password.as_bytes())
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    Ok(hash.to_string())
}

/// Check a password against a stored PHC string. Malformed hashes never verify.
pub fn verify_password(phc: &str, password: &str) -> bool {
    // The parameters embedded in the PHC string take precedence over these.
    Argon2::default()
        .verify_password(password.as_bytes(), phc)
        .is_ok()
}

fn random<const N: usize>() -> CryptoResult<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|_| CryptoError::Random)?;
    Ok(buf)
}

fn derive_key(
    password: &str,
    salt: &[u8],
    m: u32,
    t: u32,
    p: u32,
) -> CryptoResult<Zeroizing<[u8; KEY_LEN]>> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2(m, t, p)?
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    Ok(key)
}

/// Encrypt `plaintext` (the card's document JSON) for the card identified by `aad`.
///
/// This is slow by design (Argon2id); call it off the UI thread.
pub fn encrypt(plaintext: &str, password: &str, aad: &[u8]) -> CryptoResult<EncryptedPayload> {
    let salt: [u8; SALT_LEN] = random()?;
    let nonce: [u8; NONCE_LEN] = random()?;
    let key = derive_key(password, &salt, M_COST, T_COST, P_COST)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| CryptoError::Decrypt)?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext.as_bytes(),
                aad,
            },
        )
        .map_err(|_| CryptoError::Decrypt)?;
    Ok(EncryptedPayload {
        v: PAYLOAD_VERSION,
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        m_cost: M_COST,
        t_cost: T_COST,
        p_cost: P_COST,
        ciphertext: B64.encode(ciphertext),
    })
}

/// Decrypt a payload produced by [`encrypt`] with the same `password` and `aad`.
pub fn decrypt(
    payload: &EncryptedPayload,
    password: &str,
    aad: &[u8],
) -> CryptoResult<Zeroizing<String>> {
    if payload.v != PAYLOAD_VERSION {
        return Err(CryptoError::UnsupportedVersion(payload.v));
    }
    let decode = |field: &str, s: &str| {
        B64.decode(s)
            .map_err(|e| CryptoError::Malformed(format!("{field}: {e}")))
    };
    let salt = decode("salt", &payload.salt)?;
    let nonce: [u8; NONCE_LEN] = decode("nonce", &payload.nonce)?
        .try_into()
        .map_err(|_| CryptoError::Malformed("nonce length".into()))?;
    let ciphertext = decode("ciphertext", &payload.ciphertext)?;
    // Refuse absurd parameters (e.g. from a tampered row) that would exhaust memory.
    if payload.m_cost > 1024 * 1024 || payload.t_cost > 64 || payload.p_cost > 16 {
        return Err(CryptoError::Malformed(
            "argon2 parameters out of range".into(),
        ));
    }
    let key = derive_key(
        password,
        &salt,
        payload.m_cost,
        payload.t_cost,
        payload.p_cost,
    )?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| CryptoError::Decrypt)?;
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &ciphertext,
                    aad,
                },
            )
            .map_err(|_| CryptoError::Decrypt)?,
    );
    String::from_utf8(plain.to_vec())
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Malformed("plaintext is not UTF-8".into()))
}

/// Seal blob bytes (an image or attachment of an encrypted card) under a fresh random
/// key. Returns the base64url key and `nonce || ciphertext`. No key derivation is
/// involved, so this is fast; the key itself is protected by the card's encrypted
/// document. `aad` is the blob id.
pub fn seal_blob(data: &[u8], aad: &[u8]) -> CryptoResult<(Zeroizing<String>, Vec<u8>)> {
    let key = Zeroizing::new(random::<KEY_LEN>()?);
    let nonce: [u8; NONCE_LEN] = random()?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| CryptoError::Decrypt)?;
    let ciphertext = cipher
        .encrypt(&XNonce::from(nonce), Payload { msg: data, aad })
        .map_err(|_| CryptoError::Decrypt)?;
    let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok((Zeroizing::new(B64URL.encode(key.as_ref())), sealed))
}

/// Open bytes produced by [`seal_blob`] with its key and the same `aad`.
pub fn open_blob(key: &str, sealed: &[u8], aad: &[u8]) -> CryptoResult<Zeroizing<Vec<u8>>> {
    let key = Zeroizing::new(
        B64URL
            .decode(key)
            .map_err(|e| CryptoError::Malformed(format!("blob key: {e}")))?,
    );
    if sealed.len() < NONCE_LEN {
        return Err(CryptoError::Malformed("sealed blob too short".into()));
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce
        .try_into()
        .map_err(|_| CryptoError::Malformed("nonce length".into()))?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::Decrypt)?;
    cipher
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

/// Rough password strength from 0 (empty) to 4 (strong), for UI hints only.
pub fn password_strength(password: &str) -> u8 {
    let len = password.chars().count();
    if len == 0 {
        return 0;
    }
    let classes = [
        password.chars().any(|c| c.is_lowercase()),
        password.chars().any(|c| c.is_uppercase()),
        password.chars().any(|c| c.is_numeric()),
        password.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    match (len, classes) {
        (0..=5, _) => 1,
        (6..=9, 0..=2) => 1,
        (6..=9, _) => 2,
        (10..=13, 0..=1) => 2,
        (10..=13, _) => 3,
        (_, 0..=1) => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_blobs_round_trip() {
        let (key, sealed) = seal_blob(b"png bytes", b"blob-1").unwrap();
        assert!(!sealed.windows(9).any(|w| w == b"png bytes"));
        assert_eq!(
            open_blob(&key, &sealed, b"blob-1").unwrap().as_slice(),
            b"png bytes"
        );
        // Bound to its id, and a different key fails.
        assert!(open_blob(&key, &sealed, b"blob-2").is_err());
        let (other, _) = seal_blob(b"x", b"blob-1").unwrap();
        assert!(open_blob(&other, &sealed, b"blob-1").is_err());
        assert!(open_blob(&key, &sealed[..10], b"blob-1").is_err());
    }

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(&hash, "correct horse"));
        assert!(!verify_password(&hash, "wrong"));
        assert!(!verify_password("not a hash", "x"));
    }

    #[test]
    fn encryption_roundtrip_and_failures() {
        let payload = encrypt("{\"secret\":true}", "pw", b"card-1").unwrap();
        assert_eq!(
            &*decrypt(&payload, "pw", b"card-1").unwrap(),
            "{\"secret\":true}"
        );
        assert!(matches!(
            decrypt(&payload, "nope", b"card-1"),
            Err(CryptoError::Decrypt)
        ));
        // Bound to the card id.
        assert!(matches!(
            decrypt(&payload, "pw", b"card-2"),
            Err(CryptoError::Decrypt)
        ));
        // Tampered ciphertext.
        let mut bad = payload.clone();
        let mut bytes = B64.decode(&bad.ciphertext).unwrap();
        bytes[0] ^= 1;
        bad.ciphertext = B64.encode(bytes);
        assert!(matches!(
            decrypt(&bad, "pw", b"card-1"),
            Err(CryptoError::Decrypt)
        ));
        // Unknown version and hostile parameters.
        let mut v2 = payload.clone();
        v2.v = 2;
        assert!(matches!(
            decrypt(&v2, "pw", b"card-1"),
            Err(CryptoError::UnsupportedVersion(2))
        ));
        let mut huge = payload;
        huge.m_cost = u32::MAX;
        assert!(matches!(
            decrypt(&huge, "pw", b"card-1"),
            Err(CryptoError::Malformed(_))
        ));
    }

    #[test]
    fn fresh_salt_and_nonce_each_time() {
        let a = encrypt("x", "pw", b"").unwrap();
        let b = encrypt("x", "pw", b"").unwrap();
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn strength() {
        assert_eq!(password_strength(""), 0);
        assert_eq!(password_strength("abc"), 1);
        assert_eq!(password_strength("Abcdef12"), 2);
        assert_eq!(password_strength("Correct-Horse-9"), 4);
    }
}
