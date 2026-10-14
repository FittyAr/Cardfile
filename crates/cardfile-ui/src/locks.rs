//! Per-user locks: verifying lock passwords, the user key pair and per-card keys.
//!
//! Everything here is blocking (Argon2) and runs on the background executor.

use anyhow::{Context as _, Result, anyhow};
use cardfile_core::{Card, CardId, LockMode, User, UserId};
use cardfile_crypto::{CardKey, UserSecret};
use cardfile_db::{CredentialUpdate, Database, Rekey};
use cardfile_doc::Document;
use cardfile_i18n::t;
use zeroize::Zeroizing;

/// Check `password` against the user's lock password and return their unlocked
/// secret key. Users from before per-user keys get a key pair now; the returned
/// user reflects that.
pub fn unlock_user(db: &Database, user: &User, password: &str) -> Result<(User, UserSecret)> {
    let hash = user
        .effective_lock_hash()
        .ok_or_else(|| anyhow!(t("card.unlock.no_password")))?;
    if !cardfile_crypto::verify_password(hash, password) {
        return Err(anyhow!(t("card.unlock.wrong")));
    }
    let id = user.id.to_string();
    if let Some(sealed) = &user.secret_key {
        let secret = cardfile_crypto::open_user_secret(sealed, password, &id)?;
        return Ok((user.clone(), secret));
    }
    let (public, sealed, secret) = cardfile_crypto::generate_user_keys(password, &id)?;
    db.update_credentials(
        user.id,
        &CredentialUpdate {
            keys: Some((public, sealed)),
            ..Default::default()
        },
    )?;
    Ok((db.user(user.id)?, secret))
}

/// Make sure a user who just proved their lock password has a key pair (so cards
/// can be shared with them). Returns the refreshed user when keys were created.
pub fn ensure_keys(db: &Database, user: &User, password: &str) -> Result<Option<User>> {
    if user.has_keys() {
        return Ok(None);
    }
    let (user, _) = unlock_user(db, user, password)?;
    Ok(Some(user))
}

/// The content of a private or encrypted card the user unlocked, and the card key
/// when it is encrypted.
pub struct Opened {
    pub doc: Document,
    pub key: Option<CardKey>,
    /// The user, refreshed if a key pair was created.
    pub user: User,
}

/// Unlock `card` for `user` with their lock password.
///
/// Legacy encrypted cards (format v1, key derived from the owner's password) are
/// moved to a per-card key on the way, with their revisions.
pub fn open_card(db: &Database, user: &User, card: &Card, password: &str) -> Result<Opened> {
    let (user, secret) = unlock_user(db, user, password)?;
    let aad = card.id.to_string();
    let (doc, key) = match (&card.encrypted, card.lock_mode) {
        (Some(payload), LockMode::Encrypted) if payload.v == 1 => {
            if card.owner_id != user.id {
                return Err(anyhow!(t("card.unlock.wrong")));
            }
            let json = cardfile_crypto::decrypt(payload, password, aad.as_bytes())?;
            let key = CardKey::generate()?;
            let rekey = upgrade_card(db, &user, card.id, &json, password, &key)?;
            db.apply_rekey(&rekey)?;
            (Document::from_json(&json)?, Some(key))
        }
        (Some(payload), LockMode::Encrypted) => {
            let key = card_key(db, user.id, &secret, card.id)?;
            let json = cardfile_crypto::decrypt_with_key(payload, &key, aad.as_bytes())?;
            (Document::from_json(&json)?, Some(key))
        }
        _ => (Document::from_json(&card.content)?, None),
    };
    Ok(Opened { doc, key, user })
}

/// Unwrap the key of an encrypted card for `user`.
pub fn card_key(db: &Database, user: UserId, secret: &UserSecret, card: CardId) -> Result<CardKey> {
    let wrapped = db
        .wrapped_card_key(card, user)?
        .ok_or_else(|| anyhow!(t("card.unlock.no_key")))?;
    Ok(cardfile_crypto::unwrap_card_key(
        &wrapped,
        secret,
        &card.to_string(),
    )?)
}

/// Re-encrypt a legacy (v1) card and its revisions under `key`, wrapped for the owner.
fn upgrade_card(
    db: &Database,
    owner: &User,
    card: CardId,
    json: &str,
    password: &str,
    key: &CardKey,
) -> Result<Rekey> {
    let aad = card.to_string();
    let public = owner
        .public_key
        .as_deref()
        .context("the owner has no key pair")?;
    let mut rekey = Rekey::default();
    rekey.cards.push((
        card,
        cardfile_crypto::encrypt_with_key(json, key, aad.as_bytes())?,
    ));
    for rev in db.card_revisions(card)? {
        let Some(payload) = rev.encrypted.as_ref().filter(|p| p.v == 1) else {
            continue;
        };
        // A revision sealed under an older password cannot be recovered; leave it.
        let Ok(json) = cardfile_crypto::decrypt(payload, password, aad.as_bytes()) else {
            tracing::warn!("revision {} of card {card} could not be upgraded", rev.id);
            continue;
        };
        rekey.revisions.push((
            rev.id,
            cardfile_crypto::encrypt_with_key(&json, key, aad.as_bytes())?,
        ));
    }
    rekey.card_keys.push((
        card,
        owner.id,
        cardfile_crypto::wrap_card_key(key, public, &aad)?,
    ));
    Ok(rekey)
}

/// Wrap `key` for the owner and every user `card` is shared with. Fails when one of
/// them has no key pair yet (they must unlock a protected card or sign in once).
pub fn wrap_for_all(
    db: &Database,
    card: CardId,
    owner: UserId,
    key: &CardKey,
) -> Result<Vec<(UserId, String)>> {
    let mut users = vec![owner];
    users.extend(db.card_shares(card)?.into_iter().map(|s| s.user_id));
    let aad = card.to_string();
    users
        .into_iter()
        .map(|id| {
            let user = db.user(id)?;
            let public = user.public_key.as_deref().ok_or_else(|| {
                anyhow!(cardfile_i18n::t_args(
                    "share.no_keys",
                    &[("name", &user.display_name)]
                ))
            })?;
            Ok((id, cardfile_crypto::wrap_card_key(key, public, &aad)?))
        })
        .collect()
}

/// Everything to write when a user's lock password changes from `old` to `new`:
/// their secret key re-sealed and any legacy encrypted cards they own upgraded.
pub fn relock_user(db: &Database, user: &User, old: &str, new: &str) -> Result<CredentialUpdate> {
    let (user, secret) = unlock_user(db, user, old)?;
    let id = user.id.to_string();
    let sealed = cardfile_crypto::seal_user_secret(&secret, new, &id)?;
    let public = secret.public_key();
    let mut update = CredentialUpdate {
        keys: Some((public, sealed)),
        ..Default::default()
    };
    let mut all = Vec::new();
    for deleted in [false, true] {
        all.extend(db.list_cards(
            user.id,
            &cardfile_core::CardFilter {
                deleted,
                ..Default::default()
            },
        )?);
    }
    for summary in all
        .into_iter()
        .filter(|c| c.lock_mode == LockMode::Encrypted)
    {
        let card = db.card(summary.id)?;
        let Some(payload) = card.encrypted.as_ref().filter(|p| p.v == 1) else {
            continue;
        };
        let json: Zeroizing<String> =
            cardfile_crypto::decrypt(payload, old, card.id.to_string().as_bytes())?;
        let key = CardKey::generate()?;
        let rekey = upgrade_card(db, &user, card.id, &json, old, &key)?;
        update.rekey.cards.extend(rekey.cards);
        update.rekey.revisions.extend(rekey.revisions);
        update.rekey.card_keys.extend(rekey.card_keys);
    }
    Ok(update)
}

#[cfg(test)]
mod tests {
    use cardfile_core::{Actor, NewCard, SharePermission};

    use super::*;

    fn user_with_password(db: &Database, name: &str, pw: &str) -> User {
        let hash = cardfile_crypto::hash_password(pw).unwrap();
        db.create_user(name, name, Some(&hash), false).unwrap()
    }

    #[test]
    fn shared_encrypted_card_opens_with_each_password() {
        let db = Database::open_in_memory().unwrap();
        let alice = user_with_password(&db, "alice", "a-pass");
        let bob = user_with_password(&db, "bob", "b-pass");
        // Bob unlocks once, so he gets a key pair.
        let bob = ensure_keys(&db, &bob, "b-pass").unwrap().unwrap();
        let (alice, _) = unlock_user(&db, &alice, "a-pass").unwrap();
        let me = Actor::user(alice.id);
        let card = db.create_card(&me, NewCard::default()).unwrap();
        let key = CardKey::generate().unwrap();
        let payload = cardfile_crypto::encrypt_with_key(
            &Document::default().to_json(),
            &key,
            card.id.to_string().as_bytes(),
        )
        .unwrap();
        let keys = wrap_for_all(&db, card.id, alice.id, &key).unwrap();
        db.set_card_lock(
            &me,
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload),
            &keys,
            &[],
        )
        .unwrap();
        let bob_key = cardfile_crypto::wrap_card_key(
            &key,
            bob.public_key.as_deref().unwrap(),
            &card.id.to_string(),
        )
        .unwrap();
        db.share_card(&me, card.id, bob.id, SharePermission::View, Some(&bob_key))
            .unwrap();
        let card = db.card(card.id).unwrap();
        assert!(
            open_card(&db, &alice, &card, "a-pass")
                .unwrap()
                .key
                .is_some()
        );
        assert!(open_card(&db, &bob, &card, "b-pass").is_ok());
        assert!(open_card(&db, &bob, &card, "a-pass").is_err());
    }

    #[test]
    fn legacy_cards_upgrade_and_survive_password_change() {
        let db = Database::open_in_memory().unwrap();
        let alice = user_with_password(&db, "alice", "old");
        let me = Actor::user(alice.id);
        let card = db.create_card(&me, NewCard::default()).unwrap();
        let v1 = cardfile_crypto::encrypt(
            &Document::default().to_json(),
            "old",
            card.id.to_string().as_bytes(),
        )
        .unwrap();
        db.set_card_lock(
            &me,
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&v1),
            &[],
            &[],
        )
        .unwrap();
        // Changing the password upgrades the legacy card.
        let update = relock_user(&db, &alice, "old", "new").unwrap();
        assert_eq!(update.rekey.cards.len(), 1);
        db.update_credentials(
            alice.id,
            &CredentialUpdate {
                password_hash: Some(Some(cardfile_crypto::hash_password("new").unwrap())),
                ..update
            },
        )
        .unwrap();
        let alice = db.user(alice.id).unwrap();
        let card = db.card(card.id).unwrap();
        assert_eq!(card.encrypted.as_ref().unwrap().v, 2);
        assert!(open_card(&db, &alice, &card, "new").is_ok());
        assert!(open_card(&db, &alice, &card, "old").is_err());
    }
}
