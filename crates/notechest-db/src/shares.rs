//! Sharing cards with other users and the per-user wrapped keys of encrypted cards.
//!
//! Shares are synced rows: ending one soft-deletes it (`deleted_at`, `rev`). Wrapped
//! keys (`card_keys`) are the exception to the soft-delete rule: they are secrets,
//! so a key that must stop working (unshare, decryption) is hard-deleted rather than
//! left in the file for a revoked user.
//!
//! Deleting a wrapped key is not enough on its own: a revoked user may have kept the
//! unwrapped card key. When the owner unshares an encrypted card the card therefore
//! moves to a new key ([`KeyRotation`]) in the same transaction.

use std::collections::BTreeSet;

use diesel::prelude::*;
use notechest_core::{
    Actor, CardAccess, CardId, CardShare, EncryptedPayload, LockMode, RevisionId, SharePermission,
    UserId, now_millis,
};

use crate::rows::{parse_id, payload_json};
use crate::schema::{card_keys, card_revisions, card_shares, cards};
use crate::{Database, DbError, DbResult};

/// A new card key for an encrypted card, prepared by the caller (who holds the
/// unlocked current key) when the owner unshares it. See [`Database::unshare_card`].
///
/// Everything sealed under the old key is re-encrypted under the new one: the
/// current content and the revisions. Revisions left out are deleted, so no
/// ciphertext under the revoked key stays behind. Sealed images and attachments keep
/// their own per-blob keys, which live inside the (now re-encrypted) document.
#[derive(Clone, Debug)]
pub struct KeyRotation {
    /// The card's `rev` the rotation was prepared from. If the card changed since,
    /// the rotation is refused and must be prepared again.
    pub base_rev: i64,
    /// The current content, encrypted under the new key (format v2, with its key id).
    pub encrypted: EncryptedPayload,
    /// Revisions re-encrypted under the new key.
    pub revisions: Vec<(RevisionId, EncryptedPayload)>,
    /// The new key wrapped for the owner and every user who keeps access.
    pub card_keys: Vec<(UserId, String)>,
}

/// Apply `rotation` to `card_id` after `removed` lost access. Checks that it was made
/// from the current version and covers exactly the users who keep access.
fn rotate_card_key(
    conn: &mut SqliteConnection,
    actor: &Actor,
    card_id: &str,
    owner: &str,
    rotation: &KeyRotation,
    now: i64,
) -> DbResult<()> {
    let invalid = |msg: &str| DbError::from(notechest_core::Error::Invalid(msg.into()));
    let rev: i64 = cards::table.find(card_id).select(cards::rev).first(conn)?;
    if rev != rotation.base_rev {
        return Err(invalid(
            "the card changed while its key was being replaced; try again",
        ));
    }
    let payload = &rotation.encrypted;
    if payload.v < 2 || payload.key_id.is_empty() {
        return Err(invalid("a key rotation needs a keyed (v2) payload"));
    }
    // Exactly the owner and the remaining recipients, each once.
    let mut expected: BTreeSet<String> = card_shares::table
        .filter(card_shares::card_id.eq(card_id))
        .filter(card_shares::deleted_at.is_null())
        .select(card_shares::user_id)
        .load::<String>(conn)?
        .into_iter()
        .collect();
    expected.insert(owner.to_owned());
    let given: BTreeSet<String> = rotation
        .card_keys
        .iter()
        .map(|(u, _)| u.to_string())
        .collect();
    if given.len() != rotation.card_keys.len() || given != expected {
        return Err(invalid(
            "the new card key must be wrapped for the owner and every remaining user",
        ));
    }
    let revisions: Vec<String> = card_revisions::table
        .filter(card_revisions::card_id.eq(card_id))
        .filter(card_revisions::encrypted.is_not_null())
        .select(card_revisions::id)
        .load(conn)?;
    let revisions: BTreeSet<String> = revisions.into_iter().collect();
    let mut rekeyed = BTreeSet::new();
    for (id, payload) in &rotation.revisions {
        let id = id.to_string();
        if !revisions.contains(&id) || !rekeyed.insert(id.clone()) {
            return Err(invalid(
                "a re-encrypted revision does not belong to the card",
            ));
        }
        diesel::update(card_revisions::table.find(&id))
            .set(card_revisions::encrypted.eq(payload_json(Some(payload))))
            .execute(conn)?;
    }
    // Revisions still sealed under the old key: a revoked user could open them.
    let stale: Vec<&String> = revisions.difference(&rekeyed).collect();
    for chunk in stale.chunks(500) {
        diesel::delete(card_revisions::table.filter(card_revisions::id.eq_any(chunk)))
            .execute(conn)?;
    }
    diesel::update(cards::table.find(card_id))
        .set((
            cards::encrypted.eq(payload_json(Some(payload))),
            cards::updated_at.eq(now),
            cards::updated_by.eq(actor.user.to_string()),
            cards::rev.eq(cards::rev + 1),
        ))
        .execute(conn)?;
    replace_card_keys(conn, card_id, &rotation.card_keys)
}

/// `card_id`'s wrapped keys are replaced by `keys` (`(user, wrapped)`). Old keys are
/// hard-deleted on purpose (see the module docs).
pub(crate) fn replace_card_keys(
    conn: &mut SqliteConnection,
    card_id: &str,
    keys: &[(UserId, String)],
) -> DbResult<()> {
    diesel::delete(card_keys::table.filter(card_keys::card_id.eq(card_id))).execute(conn)?;
    let now = now_millis();
    for (user, wrapped) in keys {
        diesel::insert_into(card_keys::table)
            .values((
                card_keys::card_id.eq(card_id),
                card_keys::user_id.eq(user.to_string()),
                card_keys::wrapped.eq(wrapped),
                card_keys::created_at.eq(now),
            ))
            .execute(conn)?;
    }
    Ok(())
}

pub(crate) fn access_of(
    conn: &mut SqliteConnection,
    card: &str,
    user: &str,
) -> DbResult<Option<CardAccess>> {
    let row: Option<(String, Option<i64>)> = cards::table
        .find(card)
        .select((cards::owner_id, cards::deleted_at))
        .first(conn)
        .optional()?;
    let Some((owner, deleted_at)) = row else {
        return Ok(None);
    };
    if owner == user {
        return Ok(Some(CardAccess::Owner));
    }
    // A card in the owner's recycle bin is hidden from the people it is shared with
    // (as in their lists) until it is restored.
    if deleted_at.is_some() {
        return Ok(None);
    }
    let perm: Option<String> = card_shares::table
        .find((card, user))
        .filter(card_shares::deleted_at.is_null())
        .select(card_shares::permission)
        .first(conn)
        .optional()?;
    Ok(perm.map(|p| CardAccess::Shared(SharePermission::parse(&p))))
}

/// Record a sharing change on the card itself, so sync sees the card changed.
fn bump_card(conn: &mut SqliteConnection, actor: &Actor, card_id: &str, now: i64) -> DbResult<()> {
    diesel::update(cards::table.find(card_id))
        .set((
            cards::updated_at.eq(now),
            cards::updated_by.eq(actor.user.to_string()),
            cards::rev.eq(cards::rev + 1),
        ))
        .execute(conn)?;
    Ok(())
}

impl Database {
    /// What `user` may do with `card`; `None` when they cannot see it at all.
    pub fn card_access(&self, card: CardId, user: UserId) -> DbResult<Option<CardAccess>> {
        self.with(|conn| access_of(conn, &card.to_string(), &user.to_string()))
    }

    /// The users a card is shared with.
    pub fn card_shares(&self, card: CardId) -> DbResult<Vec<CardShare>> {
        self.with(|conn| {
            let rows: Vec<(String, String, String, i64)> = card_shares::table
                .filter(card_shares::card_id.eq(card.to_string()))
                .filter(card_shares::deleted_at.is_null())
                .order(card_shares::created_at.asc())
                .select((
                    card_shares::card_id,
                    card_shares::user_id,
                    card_shares::permission,
                    card_shares::created_at,
                ))
                .load(conn)?;
            rows.into_iter()
                .map(|(c, u, p, at)| {
                    Ok(CardShare {
                        card_id: parse_id(&c)?,
                        user_id: parse_id(&u)?,
                        permission: SharePermission::parse(&p),
                        created_at: at,
                    })
                })
                .collect()
        })
    }

    /// Share `card` with `user` (or change their permission). Only the owner may share.
    /// `wrapped_key` is the card key wrapped for `user` when the card is encrypted.
    pub fn share_card(
        &self,
        actor: &Actor,
        card: CardId,
        user: UserId,
        permission: SharePermission,
        wrapped_key: Option<&str>,
    ) -> DbResult<()> {
        let card_id = card.to_string();
        self.tx(|conn| {
            let row: (String, String, String) = cards::table
                .find(&card_id)
                .select((cards::owner_id, cards::lock_mode, cards::title))
                .first(conn)?;
            let (owner, lock, title) = row;
            if owner != actor.principal().to_string() {
                return Err(DbError::Domain(notechest_core::Error::PermissionDenied));
            }
            crate::cards::ensure_not_trashed(conn, &card_id)?;
            if owner == user.to_string() {
                return Err(
                    notechest_core::Error::Invalid("cannot share with the owner".into()).into(),
                );
            }
            if lock == "encrypted" && wrapped_key.is_none() {
                return Err(notechest_core::Error::Invalid(
                    "an encrypted card needs the key wrapped for the recipient".into(),
                )
                .into());
            }
            let now = now_millis();
            // Re-sharing revives a previously ended share.
            diesel::insert_into(card_shares::table)
                .values((
                    card_shares::card_id.eq(&card_id),
                    card_shares::user_id.eq(user.to_string()),
                    card_shares::permission.eq(permission.as_str()),
                    card_shares::created_at.eq(now),
                    card_shares::updated_at.eq(now),
                    card_shares::rev.eq(1),
                ))
                .on_conflict((card_shares::card_id, card_shares::user_id))
                .do_update()
                .set((
                    card_shares::permission.eq(permission.as_str()),
                    card_shares::deleted_at.eq(None::<i64>),
                    card_shares::updated_at.eq(now),
                    card_shares::rev.eq(card_shares::rev + 1),
                ))
                .execute(conn)?;
            if let Some(wrapped) = wrapped_key {
                diesel::replace_into(card_keys::table)
                    .values((
                        card_keys::card_id.eq(&card_id),
                        card_keys::user_id.eq(user.to_string()),
                        card_keys::wrapped.eq(wrapped),
                        card_keys::created_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            bump_card(conn, actor, &card_id, now)?;
            crate::activity::log(conn, actor, Some(&card_id), "share", &title)
        })
    }

    /// Stop sharing `card` with `user`. The owner may unshare anyone; a recipient may
    /// remove themselves.
    ///
    /// When the owner unshares an **encrypted** card, `rotation` is required: the
    /// removed user may still hold the unwrapped card key, so the card moves to a new
    /// key in the same transaction (see [`KeyRotation`]). Later saves must use the new
    /// key; one made with the old key is refused ([`notechest_core::Error::LockModeMismatch`]).
    /// A recipient leaving on their own passes `None`: they cannot be trusted to
    /// rewrite the ciphertext, and only their wrapped key is removed. Plain cards
    /// take `None`.
    pub fn unshare_card(
        &self,
        actor: &Actor,
        card: CardId,
        user: UserId,
        rotation: Option<&KeyRotation>,
    ) -> DbResult<()> {
        let card_id = card.to_string();
        self.tx(|conn| {
            let row: (String, String, String) = cards::table
                .find(&card_id)
                .select((cards::owner_id, cards::title, cards::lock_mode))
                .first(conn)?;
            let me = actor.principal();
            let by_owner = row.0 == me.to_string();
            if !by_owner && me != user {
                return Err(DbError::Domain(notechest_core::Error::PermissionDenied));
            }
            let now = now_millis();
            let n = diesel::update(
                card_shares::table
                    .find((&card_id, user.to_string()))
                    .filter(card_shares::deleted_at.is_null()),
            )
            .set((
                card_shares::deleted_at.eq(now),
                card_shares::updated_at.eq(now),
                card_shares::rev.eq(card_shares::rev + 1),
            ))
            .execute(conn)?;
            // A secret: removed for real (see the module docs).
            diesel::delete(card_keys::table.find((&card_id, user.to_string()))).execute(conn)?;
            if n == 0 {
                // Not shared with them: nothing to revoke, no key to replace.
                return Ok(());
            }
            let encrypted = LockMode::parse(&row.2) == Some(LockMode::Encrypted);
            match (encrypted && by_owner, rotation) {
                (true, Some(rotation)) => {
                    // Also bumps the card.
                    rotate_card_key(conn, actor, &card_id, &row.0, rotation, now)?;
                }
                (false, None) => bump_card(conn, actor, &card_id, now)?,
                (true, None) => {
                    return Err(notechest_core::Error::Invalid(
                        "unsharing an encrypted card needs a new card key".into(),
                    )
                    .into());
                }
                (false, Some(_)) => {
                    return Err(notechest_core::Error::Invalid(
                        "only the owner of an encrypted card replaces its key".into(),
                    )
                    .into());
                }
            }
            crate::activity::log(conn, actor, Some(&card_id), "unshare", &row.1)
        })
    }

    /// The card key wrapped for `user`, if the card is encrypted (format v2) and
    /// they have access.
    pub fn wrapped_card_key(&self, card: CardId, user: UserId) -> DbResult<Option<String>> {
        self.with(|conn| {
            Ok(card_keys::table
                .find((card.to_string(), user.to_string()))
                .select(card_keys::wrapped)
                .first(conn)
                .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use notechest_core::{Actor, CardAccess, CardFilter, LockMode, NewCard, SharePermission};

    use super::KeyRotation;
    use crate::DbError;
    use crate::test_util::*;

    #[test]
    fn share_view_and_edit() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let b = Actor::user(bob.id);
        let card = db
            .create_card(
                &a,
                NewCard {
                    title: "Plan".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(db.card_access(card.id, bob.id).unwrap(), None);
        // Only the owner may share.
        assert!(
            db.share_card(&b, card.id, bob.id, SharePermission::Edit, None)
                .is_err()
        );
        db.share_card(&a, card.id, bob.id, SharePermission::View, None)
            .unwrap();
        assert_eq!(
            db.card_access(card.id, bob.id).unwrap(),
            Some(CardAccess::Shared(SharePermission::View))
        );
        let shared = CardFilter {
            shared_with_me: true,
            ..Default::default()
        };
        let list = db.list_cards(bob.id, &shared).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].owner_id, alice.id);
        assert_eq!(list[0].access, CardAccess::Shared(SharePermission::View));
        // Bob's own list stays empty; Alice sees the card as shared.
        assert!(
            db.list_cards(bob.id, &CardFilter::default())
                .unwrap()
                .is_empty()
        );
        let mine = db.list_cards(alice.id, &CardFilter::default()).unwrap();
        assert!(mine[0].is_shared);
        assert_eq!(mine[0].access, CardAccess::Owner);

        db.share_card(&a, card.id, bob.id, SharePermission::Edit, None)
            .unwrap();
        assert_eq!(
            db.card_shares(card.id).unwrap()[0].permission,
            SharePermission::Edit
        );
        // Deleted cards disappear from the shared list.
        db.delete_card(&a, card.id).unwrap();
        assert!(db.list_cards(bob.id, &shared).unwrap().is_empty());
        db.restore_card(&a, card.id).unwrap();
        // A recipient may leave a share.
        db.unshare_card(&b, card.id, bob.id, None).unwrap();
        assert_eq!(db.card_access(card.id, bob.id).unwrap(), None);
    }

    #[test]
    fn unsharing_is_a_soft_delete_that_bumps_the_card() {
        use diesel::prelude::*;
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let card = db.create_card(&a, NewCard::default()).unwrap();
        db.share_card(&a, card.id, bob.id, SharePermission::Edit, None)
            .unwrap();
        let shared = db.card(card.id).unwrap();
        assert_eq!(shared.rev, card.rev + 1);
        db.unshare_card(&a, card.id, bob.id, None).unwrap();
        let unshared = db.card(card.id).unwrap();
        assert_eq!(unshared.rev, shared.rev + 1);
        assert!(db.card_shares(card.id).unwrap().is_empty());
        assert!(!db.list_cards(alice.id, &CardFilter::default()).unwrap()[0].is_shared);
        let shared_with_bob = CardFilter {
            shared_with_me: true,
            ..Default::default()
        };
        assert!(db.list_cards(bob.id, &shared_with_bob).unwrap().is_empty());
        // The row stays as a tombstone.
        let (deleted, rev): (Option<i64>, i64) = db
            .with(|conn| {
                Ok(crate::schema::card_shares::table
                    .select((
                        crate::schema::card_shares::deleted_at,
                        crate::schema::card_shares::rev,
                    ))
                    .first(conn)?)
            })
            .unwrap();
        assert!(deleted.is_some());
        assert_eq!(rev, 2);
        // Sharing again revives it.
        db.share_card(&a, card.id, bob.id, SharePermission::View, None)
            .unwrap();
        assert_eq!(
            db.card_access(card.id, bob.id).unwrap(),
            Some(CardAccess::Shared(SharePermission::View))
        );
        assert_eq!(db.list_cards(bob.id, &shared_with_bob).unwrap().len(), 1);
    }

    #[test]
    fn encrypted_shares_need_a_wrapped_key() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let card = db.create_card(&a, NewCard::default()).unwrap();
        let payload = notechest_core::EncryptedPayload {
            v: 2,
            salt: String::new(),
            nonce: "n".into(),
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
            ciphertext: "c".into(),
            key_id: String::new(),
        };
        db.set_card_lock(
            &a,
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload),
            &[(alice.id, "owner-key".into())],
            &[],
        )
        .unwrap();
        assert!(
            db.share_card(&a, card.id, bob.id, SharePermission::View, None)
                .is_err()
        );
        db.share_card(&a, card.id, bob.id, SharePermission::View, Some("wrapped"))
            .unwrap();
        assert_eq!(
            db.wrapped_card_key(card.id, bob.id).unwrap().as_deref(),
            Some("wrapped")
        );
        // The owner cannot just drop Bob's key: the card needs a new one.
        assert!(db.unshare_card(&a, card.id, bob.id, None).is_err());
        assert_eq!(
            db.wrapped_card_key(card.id, bob.id).unwrap().as_deref(),
            Some("wrapped")
        );
        let rotation = KeyRotation {
            base_rev: db.card(card.id).unwrap().rev,
            encrypted: notechest_core::EncryptedPayload {
                key_id: "new-key".into(),
                ..payload
            },
            revisions: vec![],
            card_keys: vec![(alice.id, "owner-key-2".into())],
        };
        db.unshare_card(&a, card.id, bob.id, Some(&rotation))
            .unwrap();
        assert_eq!(db.wrapped_card_key(card.id, bob.id).unwrap(), None);
        assert_eq!(
            db.wrapped_card_key(card.id, alice.id).unwrap().as_deref(),
            Some("owner-key-2")
        );
    }

    /// Real keys: a user who kept the unwrapped card key from before they were
    /// removed cannot read anything written (or kept) after the unshare.
    #[test]
    fn unsharing_an_encrypted_card_rotates_its_key() {
        use notechest_crypto::{
            CardKey, decrypt_with_key, encrypt_with_key, generate_user_keys, reencrypt_with_key,
            unwrap_card_key, wrap_card_key,
        };

        use crate::ContentUpdate;

        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let carol = db.create_user("carol", "Carol", None, false).unwrap();
        let (alice_pub, _, alice_secret) = generate_user_keys("a", &alice.id.to_string()).unwrap();
        let (bob_pub, _, bob_secret) = generate_user_keys("b", &bob.id.to_string()).unwrap();
        let (carol_pub, _, carol_secret) = generate_user_keys("c", &carol.id.to_string()).unwrap();
        let a = Actor::user(alice.id);
        let card = db.create_card(&a, NewCard::default()).unwrap();
        let aad = card.id.to_string();
        let seal = |json: &str, key: &CardKey| encrypt_with_key(json, key, aad.as_bytes()).unwrap();
        let save = |json: &str, key: &CardKey| {
            db.update_card_content(
                &a,
                card.id,
                ContentUpdate {
                    title: "t".into(),
                    encrypted: Some(seal(json, key)),
                    ..Default::default()
                },
                10,
                0,
            )
        };

        // Encrypted under K1 and shared with Bob and Carol, with some history.
        let k1 = CardKey::generate().unwrap();
        db.set_card_lock(
            &a,
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&seal("v1", &k1)),
            &[(alice.id, wrap_card_key(&k1, &alice_pub, &aad).unwrap())],
            &[],
        )
        .unwrap();
        for (user, public) in [(bob.id, &bob_pub), (carol.id, &carol_pub)] {
            let wrapped = wrap_card_key(&k1, public, &aad).unwrap();
            db.share_card(&a, card.id, user, SharePermission::Edit, Some(&wrapped))
                .unwrap();
        }
        save("v2", &k1).unwrap();
        save("v3", &k1).unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 2);
        // Bob unwraps K1 and keeps it.
        let bob_kept = unwrap_card_key(
            &db.wrapped_card_key(card.id, bob.id).unwrap().unwrap(),
            &bob_secret,
            &aad,
        )
        .unwrap();

        // Alice removes Bob: everything moves to K2, wrapped for Alice and Carol.
        let k2 = CardKey::generate().unwrap();
        let current = db.card(card.id).unwrap();
        let revisions = db.card_revisions(card.id).unwrap();
        let rotation = KeyRotation {
            base_rev: current.rev,
            encrypted: reencrypt_with_key(
                current.encrypted.as_ref().unwrap(),
                &k1,
                &k2,
                aad.as_bytes(),
            )
            .unwrap(),
            // One revision is re-encrypted; the other is left out and must go.
            revisions: vec![(
                revisions[0].id,
                reencrypt_with_key(
                    revisions[0].encrypted.as_ref().unwrap(),
                    &k1,
                    &k2,
                    aad.as_bytes(),
                )
                .unwrap(),
            )],
            card_keys: vec![
                (alice.id, wrap_card_key(&k2, &alice_pub, &aad).unwrap()),
                (carol.id, wrap_card_key(&k2, &carol_pub, &aad).unwrap()),
            ],
        };
        // A rotation that still includes Bob, or forgets Carol, is refused.
        for keys in [
            vec![rotation.card_keys[0].clone()],
            vec![
                rotation.card_keys[0].clone(),
                rotation.card_keys[1].clone(),
                (bob.id, wrap_card_key(&k2, &bob_pub, &aad).unwrap()),
            ],
        ] {
            let bad = KeyRotation {
                card_keys: keys,
                ..rotation.clone()
            };
            assert!(db.unshare_card(&a, card.id, bob.id, Some(&bad)).is_err());
        }
        // So is one prepared from an older version of the card.
        let stale = KeyRotation {
            base_rev: rotation.base_rev - 1,
            ..rotation.clone()
        };
        assert!(db.unshare_card(&a, card.id, bob.id, Some(&stale)).is_err());
        assert!(db.wrapped_card_key(card.id, bob.id).unwrap().is_some());
        db.unshare_card(&a, card.id, bob.id, Some(&rotation))
            .unwrap();
        assert_eq!(db.wrapped_card_key(card.id, bob.id).unwrap(), None);
        assert_eq!(db.card(card.id).unwrap().rev, current.rev + 1);

        // A save with the old key (an editor that missed the rotation) is refused.
        assert!(matches!(
            save("stale", &k1).unwrap_err(),
            DbError::Domain(notechest_core::Error::LockModeMismatch)
        ));
        // Alice writes after the unshare with the new key.
        save("after unshare", &k2).unwrap();
        let after = db.card(card.id).unwrap();
        let payload = after.encrypted.as_ref().unwrap();
        assert!(decrypt_with_key(payload, &bob_kept, aad.as_bytes()).is_err());
        // Nothing left in the history opens with Bob's old key either.
        let history = db.card_revisions(card.id).unwrap();
        assert!(!history.is_empty());
        for rev in &history {
            let p = rev.encrypted.as_ref().unwrap();
            assert!(decrypt_with_key(p, &bob_kept, aad.as_bytes()).is_err());
        }
        assert!(
            !history.iter().any(|r| r.id == revisions[1].id),
            "the revision left under the old key was not purged"
        );
        // Carol and Alice open the new content with their own keys.
        for (user, secret) in [(carol.id, &carol_secret), (alice.id, &alice_secret)] {
            let wrapped = db.wrapped_card_key(card.id, user).unwrap().unwrap();
            let key = unwrap_card_key(&wrapped, secret, &aad).unwrap();
            assert_eq!(
                &*decrypt_with_key(payload, &key, aad.as_bytes()).unwrap(),
                "after unshare"
            );
        }
    }

    #[test]
    fn recipients_leaving_an_encrypted_card_do_not_rotate_it() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let b = Actor::user(bob.id);
        let card = db.create_card(&a, NewCard::default()).unwrap();
        let payload = notechest_core::EncryptedPayload {
            v: 2,
            salt: String::new(),
            nonce: "n".into(),
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
            ciphertext: "c".into(),
            key_id: "k1".into(),
        };
        db.set_card_lock(
            &a,
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload),
            &[(alice.id, "owner-key".into())],
            &[],
        )
        .unwrap();
        db.share_card(&a, card.id, bob.id, SharePermission::View, Some("bob-key"))
            .unwrap();
        // A viewer may not rewrite the ciphertext on the way out.
        let rotation = KeyRotation {
            base_rev: db.card(card.id).unwrap().rev,
            encrypted: payload.clone(),
            revisions: vec![],
            card_keys: vec![(alice.id, "evil".into())],
        };
        assert!(
            db.unshare_card(&b, card.id, bob.id, Some(&rotation))
                .is_err()
        );
        db.unshare_card(&b, card.id, bob.id, None).unwrap();
        assert_eq!(db.wrapped_card_key(card.id, bob.id).unwrap(), None);
        assert_eq!(
            db.wrapped_card_key(card.id, alice.id).unwrap().as_deref(),
            Some("owner-key")
        );
        assert_eq!(db.card(card.id).unwrap().encrypted, Some(payload));
    }
}
