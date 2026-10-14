//! Per-user keys and per-card keys (envelope format version 2).
//!
//! - Every user has an X25519 key pair. The public key is stored in the clear; the
//!   secret key is encrypted under the user's lock password (Argon2id + XChaCha20,
//!   the version 1 envelope with the user id as associated data).
//! - Every encrypted card has a random 256-bit [`CardKey`]. Its content (and its
//!   revisions) are encrypted with that key, and the key is wrapped once for each user
//!   with access (the owner and the people it is shared with): an ephemeral X25519
//!   exchange with the recipient's public key, BLAKE3 key derivation and
//!   XChaCha20-Poly1305, bound to the card id.
//!
//! Changing a lock password therefore only re-encrypts one small secret key, and a
//! shared encrypted card opens with each recipient's own password.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use cardfile_core::EncryptedPayload;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::{CryptoError, CryptoResult, KEY_LEN, NONCE_LEN, random};

/// Envelope version of content encrypted with a [`CardKey`].
pub const KEYED_PAYLOAD_VERSION: u32 = 2;

const WRAP_CONTEXT: &str = "Cardfile 2 card key wrap v1";

/// A user's unlocked X25519 secret key. Zeroed on drop.
pub struct UserSecret(Zeroizing<[u8; KEY_LEN]>);

impl std::fmt::Debug for UserSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UserSecret(..)")
    }
}

impl UserSecret {
    /// The matching public key, base64.
    pub fn public_key(&self) -> String {
        let secret = StaticSecret::from(*self.0);
        B64.encode(PublicKey::from(&secret).as_bytes())
    }
}

/// The random key of one encrypted card. Zeroed on drop.
#[derive(Clone)]
pub struct CardKey(Zeroizing<[u8; KEY_LEN]>);

impl std::fmt::Debug for CardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CardKey(..)")
    }
}

impl CardKey {
    /// A fresh random key.
    pub fn generate() -> CryptoResult<Self> {
        Ok(Self(Zeroizing::new(random()?)))
    }
}

/// A new key pair for `user_id`, the secret sealed under `password`.
///
/// Returns the public key (base64), the sealed secret and the unlocked secret.
/// Slow (Argon2id): call it off the UI thread.
pub fn generate_user_keys(
    password: &str,
    user_id: &str,
) -> CryptoResult<(String, EncryptedPayload, UserSecret)> {
    let secret = UserSecret(Zeroizing::new(random()?));
    let sealed = seal_user_secret(&secret, password, user_id)?;
    Ok((secret.public_key(), sealed, secret))
}

/// Encrypt a secret key under a (new) lock password.
pub fn seal_user_secret(
    secret: &UserSecret,
    password: &str,
    user_id: &str,
) -> CryptoResult<EncryptedPayload> {
    let encoded = Zeroizing::new(B64.encode(*secret.0));
    crate::encrypt(&encoded, password, user_id.as_bytes())
}

/// Decrypt a user's secret key with their lock password.
pub fn open_user_secret(
    sealed: &EncryptedPayload,
    password: &str,
    user_id: &str,
) -> CryptoResult<UserSecret> {
    let encoded = crate::decrypt(sealed, password, user_id.as_bytes())?;
    let bytes = Zeroizing::new(
        B64.decode(encoded.as_bytes())
            .map_err(|e| CryptoError::Malformed(format!("secret key: {e}")))?,
    );
    let key: [u8; KEY_LEN] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Malformed("secret key length".into()))?;
    Ok(UserSecret(Zeroizing::new(key)))
}

fn wrap_key(shared: &[u8; 32], eph_public: &[u8; 32], recipient: &[u8; 32]) -> [u8; KEY_LEN] {
    let mut material = Zeroizing::new([0u8; 96]);
    material[..32].copy_from_slice(shared);
    material[32..64].copy_from_slice(eph_public);
    material[64..].copy_from_slice(recipient);
    blake3::derive_key(WRAP_CONTEXT, material.as_ref())
}

fn decode_public(public_b64: &str) -> CryptoResult<[u8; 32]> {
    B64.decode(public_b64)
        .map_err(|e| CryptoError::Malformed(format!("public key: {e}")))?
        .try_into()
        .map_err(|_| CryptoError::Malformed("public key length".into()))
}

/// Wrap `key` for the holder of `recipient_public` (base64). `card_id` is bound as
/// associated data. Returns base64 of `ephemeral public || nonce || ciphertext`.
pub fn wrap_card_key(key: &CardKey, recipient_public: &str, card_id: &str) -> CryptoResult<String> {
    let recipient = decode_public(recipient_public)?;
    let eph = StaticSecret::from(random::<32>()?);
    let eph_public = PublicKey::from(&eph);
    let shared = eph.diffie_hellman(&PublicKey::from(recipient));
    if !shared.was_contributory() {
        return Err(CryptoError::Malformed("degenerate public key".into()));
    }
    let wrap = Zeroizing::new(wrap_key(
        shared.as_bytes(),
        eph_public.as_bytes(),
        &recipient,
    ));
    let nonce: [u8; NONCE_LEN] = random()?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(wrap.as_ref()).map_err(|_| CryptoError::Decrypt)?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: key.0.as_ref(),
                aad: card_id.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Decrypt)?;
    let mut out = Vec::with_capacity(32 + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(eph_public.as_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(B64.encode(out))
}

/// Unwrap a card key produced by [`wrap_card_key`] for this user.
pub fn unwrap_card_key(wrapped: &str, secret: &UserSecret, card_id: &str) -> CryptoResult<CardKey> {
    let bytes = B64
        .decode(wrapped)
        .map_err(|e| CryptoError::Malformed(format!("wrapped key: {e}")))?;
    if bytes.len() < 32 + NONCE_LEN {
        return Err(CryptoError::Malformed("wrapped key too short".into()));
    }
    let (eph_public, rest) = bytes.split_at(32);
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let eph_public: [u8; 32] = eph_public
        .try_into()
        .map_err(|_| CryptoError::Malformed("ephemeral key length".into()))?;
    let nonce: [u8; NONCE_LEN] = nonce
        .try_into()
        .map_err(|_| CryptoError::Malformed("nonce length".into()))?;
    let me = StaticSecret::from(*secret.0);
    let my_public = PublicKey::from(&me);
    let shared = me.diffie_hellman(&PublicKey::from(eph_public));
    let wrap = Zeroizing::new(wrap_key(
        shared.as_bytes(),
        &eph_public,
        my_public.as_bytes(),
    ));
    let cipher =
        XChaCha20Poly1305::new_from_slice(wrap.as_ref()).map_err(|_| CryptoError::Decrypt)?;
    let key = Zeroizing::new(
        cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: ciphertext,
                    aad: card_id.as_bytes(),
                },
            )
            .map_err(|_| CryptoError::Decrypt)?,
    );
    let key: [u8; KEY_LEN] = key
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Malformed("card key length".into()))?;
    Ok(CardKey(Zeroizing::new(key)))
}

/// Encrypt card content with its key (envelope version 2). Fast: no key derivation.
pub fn encrypt_with_key(
    plaintext: &str,
    key: &CardKey,
    aad: &[u8],
) -> CryptoResult<EncryptedPayload> {
    let nonce: [u8; NONCE_LEN] = random()?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.0.as_ref()).map_err(|_| CryptoError::Decrypt)?;
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
        v: KEYED_PAYLOAD_VERSION,
        salt: String::new(),
        nonce: B64.encode(nonce),
        m_cost: 0,
        t_cost: 0,
        p_cost: 0,
        ciphertext: B64.encode(ciphertext),
    })
}

/// Decrypt content produced by [`encrypt_with_key`].
pub fn decrypt_with_key(
    payload: &EncryptedPayload,
    key: &CardKey,
    aad: &[u8],
) -> CryptoResult<Zeroizing<String>> {
    if payload.v != KEYED_PAYLOAD_VERSION {
        return Err(CryptoError::UnsupportedVersion(payload.v));
    }
    let nonce: [u8; NONCE_LEN] = B64
        .decode(&payload.nonce)
        .map_err(|e| CryptoError::Malformed(format!("nonce: {e}")))?
        .try_into()
        .map_err(|_| CryptoError::Malformed("nonce length".into()))?;
    let ciphertext = B64
        .decode(&payload.ciphertext)
        .map_err(|e| CryptoError::Malformed(format!("ciphertext: {e}")))?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.0.as_ref()).map_err(|_| CryptoError::Decrypt)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_keys_round_trip() {
        let (public, sealed, secret) = generate_user_keys("pw", "user-1").unwrap();
        assert_eq!(secret.public_key(), public);
        let opened = open_user_secret(&sealed, "pw", "user-1").unwrap();
        assert_eq!(opened.public_key(), public);
        assert!(open_user_secret(&sealed, "bad", "user-1").is_err());
        assert!(open_user_secret(&sealed, "pw", "user-2").is_err());
        // Re-sealing under a new password keeps the same key pair.
        let resealed = seal_user_secret(&opened, "new", "user-1").unwrap();
        assert_eq!(
            open_user_secret(&resealed, "new", "user-1")
                .unwrap()
                .public_key(),
            public
        );
    }

    #[test]
    fn card_keys_are_shared_per_recipient() {
        let (alice_pub, _, alice) = generate_user_keys("a", "alice").unwrap();
        let (bob_pub, _, bob) = generate_user_keys("b", "bob").unwrap();
        let key = CardKey::generate().unwrap();
        let payload = encrypt_with_key("{\"x\":1}", &key, b"card-1").unwrap();
        assert_eq!(payload.v, KEYED_PAYLOAD_VERSION);

        for (public, secret) in [(&alice_pub, &alice), (&bob_pub, &bob)] {
            let wrapped = wrap_card_key(&key, public, "card-1").unwrap();
            let unwrapped = unwrap_card_key(&wrapped, secret, "card-1").unwrap();
            assert_eq!(
                &*decrypt_with_key(&payload, &unwrapped, b"card-1").unwrap(),
                "{\"x\":1}"
            );
            // Bound to the card, and useless to anyone else.
            assert!(unwrap_card_key(&wrapped, secret, "card-2").is_err());
        }
        let for_bob = wrap_card_key(&key, &bob_pub, "card-1").unwrap();
        assert!(unwrap_card_key(&for_bob, &alice, "card-1").is_err());
        // Tampering is detected.
        assert!(decrypt_with_key(&payload, &key, b"card-2").is_err());
        let other = CardKey::generate().unwrap();
        assert!(decrypt_with_key(&payload, &other, b"card-1").is_err());
        assert!(unwrap_card_key("AAAA", &alice, "card-1").is_err());
    }

    #[test]
    fn versions_do_not_mix() {
        let key = CardKey::generate().unwrap();
        let v2 = encrypt_with_key("x", &key, b"").unwrap();
        assert!(matches!(
            crate::decrypt(&v2, "pw", b""),
            Err(CryptoError::UnsupportedVersion(2))
        ));
        let v1 = crate::encrypt("x", "pw", b"").unwrap();
        assert!(matches!(
            decrypt_with_key(&v1, &key, b""),
            Err(CryptoError::UnsupportedVersion(1))
        ));
    }
}
