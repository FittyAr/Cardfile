//! Sharing cards with other users and the per-user wrapped keys of encrypted cards.

use cardfile_core::{Actor, CardAccess, CardId, CardShare, SharePermission, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::parse_id;
use crate::schema::{card_keys, card_shares, cards};
use crate::{Database, DbError, DbResult};

/// `card_id`'s wrapped keys are replaced by `keys` (`(user, wrapped)`).
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
    let owner: Option<String> = cards::table
        .find(card)
        .select(cards::owner_id)
        .first(conn)
        .optional()?;
    let Some(owner) = owner else {
        return Ok(None);
    };
    if owner == user {
        return Ok(Some(CardAccess::Owner));
    }
    let perm: Option<String> = card_shares::table
        .find((card, user))
        .select(card_shares::permission)
        .first(conn)
        .optional()?;
    Ok(perm.map(|p| CardAccess::Shared(SharePermission::parse(&p))))
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
                return Err(DbError::Domain(cardfile_core::Error::PermissionDenied));
            }
            if owner == user.to_string() {
                return Err(
                    cardfile_core::Error::Invalid("cannot share with the owner".into()).into(),
                );
            }
            if lock == "encrypted" && wrapped_key.is_none() {
                return Err(cardfile_core::Error::Invalid(
                    "an encrypted card needs the key wrapped for the recipient".into(),
                )
                .into());
            }
            diesel::insert_into(card_shares::table)
                .values((
                    card_shares::card_id.eq(&card_id),
                    card_shares::user_id.eq(user.to_string()),
                    card_shares::permission.eq(permission.as_str()),
                    card_shares::created_at.eq(now_millis()),
                ))
                .on_conflict((card_shares::card_id, card_shares::user_id))
                .do_update()
                .set(card_shares::permission.eq(permission.as_str()))
                .execute(conn)?;
            if let Some(wrapped) = wrapped_key {
                diesel::replace_into(card_keys::table)
                    .values((
                        card_keys::card_id.eq(&card_id),
                        card_keys::user_id.eq(user.to_string()),
                        card_keys::wrapped.eq(wrapped),
                        card_keys::created_at.eq(now_millis()),
                    ))
                    .execute(conn)?;
            }
            crate::activity::log(conn, actor, Some(&card_id), "share", &title)
        })
    }

    /// Stop sharing `card` with `user`. The owner may unshare anyone; a recipient may
    /// remove themselves.
    pub fn unshare_card(&self, actor: &Actor, card: CardId, user: UserId) -> DbResult<()> {
        let card_id = card.to_string();
        self.tx(|conn| {
            let row: (String, String) = cards::table
                .find(&card_id)
                .select((cards::owner_id, cards::title))
                .first(conn)?;
            let me = actor.principal();
            if row.0 != me.to_string() && me != user {
                return Err(DbError::Domain(cardfile_core::Error::PermissionDenied));
            }
            diesel::delete(card_shares::table.find((&card_id, user.to_string()))).execute(conn)?;
            diesel::delete(card_keys::table.find((&card_id, user.to_string()))).execute(conn)?;
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
    use cardfile_core::{Actor, CardAccess, CardFilter, LockMode, NewCard, SharePermission};

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
        db.unshare_card(&b, card.id, bob.id).unwrap();
        assert_eq!(db.card_access(card.id, bob.id).unwrap(), None);
    }

    #[test]
    fn encrypted_shares_need_a_wrapped_key() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let card = db
            .create_card(
                &a,
                NewCard {
                    lock_mode: LockMode::Encrypted,
                    ..Default::default()
                },
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
        db.unshare_card(&a, card.id, bob.id).unwrap();
        assert_eq!(db.wrapped_card_key(card.id, bob.id).unwrap(), None);
    }
}
