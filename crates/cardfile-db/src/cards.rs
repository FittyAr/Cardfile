use std::collections::HashMap;

use cardfile_core::{
    Actor, Card, CardId, CardRevision, EncryptedPayload, LockMode, NewCard, RevisionId, TagId,
    UserId, now_millis, order,
};
use diesel::prelude::*;

use crate::rows::{CardRow, RevisionRow, parse_id, payload_json};
use crate::schema::{activity, blobs, card_blobs, card_revisions, card_tags, cards};
use crate::{Database, DbError, DbResult};

/// Changes to a card's title and content.
#[derive(Clone, Debug, Default)]
pub struct ContentUpdate {
    pub title: String,
    /// Document JSON; empty for encrypted cards.
    pub content: String,
    pub plain_text: String,
    /// Replaces the ciphertext when the card is encrypted.
    pub encrypted: Option<EncryptedPayload>,
    /// Blob ids referenced by the document (images, attachments).
    pub blob_ids: Vec<String>,
}

/// Fail unless `actor` may edit `card` (owner, or shared with edit permission).
pub(crate) fn ensure_can_edit(
    conn: &mut SqliteConnection,
    actor: &Actor,
    card: &str,
) -> DbResult<()> {
    match crate::shares::access_of(conn, card, &actor.principal().to_string())? {
        Some(a) if a.can_edit() => Ok(()),
        Some(_) => Err(DbError::Domain(cardfile_core::Error::PermissionDenied)),
        None => Err(DbError::Query(diesel::result::Error::NotFound)),
    }
}

/// Fail unless `actor` owns `card`.
pub(crate) fn ensure_owner(conn: &mut SqliteConnection, actor: &Actor, card: &str) -> DbResult<()> {
    match crate::shares::access_of(conn, card, &actor.principal().to_string())? {
        Some(a) if a.is_owner() => Ok(()),
        Some(_) => Err(DbError::Domain(cardfile_core::Error::PermissionDenied)),
        None => Err(DbError::Query(diesel::result::Error::NotFound)),
    }
}

/// Fail unless `update` fits the card's current protection: an encrypted card takes
/// only ciphertext (and no plain content), any other card only plain content.
///
/// Checked inside the saving transaction, so a save prepared before the card was
/// encrypted (e.g. a racing AI edit) cannot write plaintext into it, and one prepared
/// before it was decrypted cannot leave ciphertext without keys behind.
fn ensure_payload_matches(lock_mode: &str, update: &ContentUpdate) -> DbResult<()> {
    let ok = if LockMode::parse(lock_mode) == Some(LockMode::Encrypted) {
        update.encrypted.is_some() && update.content.is_empty() && update.plain_text.is_empty()
    } else {
        update.encrypted.is_none()
    };
    if ok {
        Ok(())
    } else {
        Err(cardfile_core::Error::LockModeMismatch.into())
    }
}

/// Blank the summaries (card titles) of the activity entries of cards being purged,
/// so a purged card leaves no readable trace in the log.
fn scrub_activity(conn: &mut SqliteConnection, card_ids: &[String]) -> DbResult<()> {
    for chunk in card_ids.chunks(500) {
        diesel::update(activity::table.filter(activity::card_id.eq_any(chunk)))
            .set(activity::summary.eq(""))
            .execute(conn)?;
    }
    Ok(())
}

fn title_of(conn: &mut SqliteConnection, card: &str) -> DbResult<String> {
    Ok(cards::table.find(card).select(cards::title).first(conn)?)
}

impl Database {
    /// Create a card for `actor`'s principal, at the top of their manual order.
    pub fn create_card(&self, actor: &Actor, new: NewCard) -> DbResult<Card> {
        let owner = actor.principal();
        self.tx(|conn| {
            // New cards go on top: generate a key before the current minimum.
            let first: Option<String> = cards::table
                .filter(cards::owner_id.eq(owner.to_string()))
                .select(diesel::dsl::min(cards::sort_key))
                .first(conn)?;
            let sort_key = order::key_between(None, first.as_deref());
            let now = now_millis();
            let row = CardRow {
                id: CardId::new().to_string(),
                owner_id: owner.to_string(),
                title: new.title,
                content: new.content,
                plain_text: new.plain_text,
                lock_mode: new.lock_mode.as_str().to_owned(),
                encrypted: None,
                is_pinned: false,
                is_favorite: false,
                sort_key,
                created_at: now,
                updated_at: now,
                deleted_at: None,
                rev: 1,
                updated_by: Some(actor.user.to_string()),
            };
            let row: CardRow = diesel::insert_into(cards::table)
                .values(&row)
                .returning(CardRow::as_returning())
                .get_result(conn)?;
            crate::activity::log(conn, actor, Some(&row.id), "create", &row.title)?;
            row.into_card(Vec::new())
        })
    }

    pub fn card(&self, id: CardId) -> DbResult<Card> {
        self.with(|conn| load_card(conn, id))
    }

    /// Save new content, snapshotting the previous version when `keep_revisions > 0`.
    ///
    /// Snapshots are throttled: a new one is taken only if the newest is older than
    /// `snapshot_every_ms`, so continuous typing does not flood the history.
    pub fn update_card_content(
        &self,
        actor: &Actor,
        id: CardId,
        update: ContentUpdate,
        keep_revisions: u32,
        snapshot_every_ms: i64,
    ) -> DbResult<Card> {
        self.tx(|conn| {
            ensure_can_edit(conn, actor, &id.to_string())?;
            let old: CardRow = cards::table
                .find(id.to_string())
                .select(CardRow::as_select())
                .first(conn)?;
            ensure_payload_matches(&old.lock_mode, &update)?;
            let now = now_millis();
            if keep_revisions > 0 {
                let newest: Option<i64> = card_revisions::table
                    .filter(card_revisions::card_id.eq(id.to_string()))
                    .select(diesel::dsl::max(card_revisions::created_at))
                    .first(conn)?;
                let changed = old.content != update.content
                    || old.title != update.title
                    || old.encrypted != payload_json(update.encrypted.as_ref());
                if changed && newest.is_none_or(|t| now - t >= snapshot_every_ms) {
                    insert_revision(conn, &old, now)?;
                    prune_revisions(conn, id, keep_revisions)?;
                }
            }
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::title.eq(&update.title),
                    cards::content.eq(&update.content),
                    cards::plain_text.eq(&update.plain_text),
                    cards::encrypted.eq(payload_json(update.encrypted.as_ref())),
                    cards::updated_at.eq(now),
                    cards::updated_by.eq(actor.user.to_string()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            add_card_blobs(conn, id, &update.blob_ids)?;
            crate::activity::log(conn, actor, Some(&id.to_string()), "edit", &update.title)?;
            load_card(conn, id)
        })
    }

    /// Change the protection level. For [`LockMode::Encrypted`] pass the ciphertext,
    /// an empty `content` and the card key wrapped for the owner and every user it is
    /// shared with (`card_keys`); when leaving encryption pass the decrypted content.
    /// `blob_ids` are the blob references of the new content. Owner only.
    ///
    /// Entering encryption drops the plaintext history and images, and then
    /// checkpoints the WAL so the replaced plaintext does not linger in it (freed
    /// pages are zeroed by `secure_delete`, FTS entries by FTS5 `secure-delete`).
    ///
    /// Leaving encryption drops the card keys and, with them, the encrypted
    /// revisions: they could no longer be decrypted, and keeping the keys only for
    /// old snapshots would leave key material for a card that is no longer
    /// encrypted. The decrypted current content is kept.
    #[allow(clippy::too_many_arguments)]
    pub fn set_card_lock(
        &self,
        actor: &Actor,
        id: CardId,
        mode: LockMode,
        content: &str,
        plain_text: &str,
        encrypted: Option<&EncryptedPayload>,
        card_keys: &[(UserId, String)],
        blob_ids: &[String],
    ) -> DbResult<Card> {
        if (mode == LockMode::Encrypted) != encrypted.is_some() {
            return Err(cardfile_core::Error::Invalid(
                "encrypted cards need a payload, others must not have one".into(),
            )
            .into());
        }
        let card = self.tx(|conn| {
            ensure_owner(conn, actor, &id.to_string())?;
            let was_encrypted = cards::table
                .find(id.to_string())
                .select(cards::lock_mode)
                .first::<String>(conn)?
                == LockMode::Encrypted.as_str();
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::lock_mode.eq(mode.as_str()),
                    cards::content.eq(content),
                    cards::plain_text.eq(plain_text),
                    cards::encrypted.eq(payload_json(encrypted)),
                    cards::updated_at.eq(now_millis()),
                    cards::updated_by.eq(actor.user.to_string()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            let keys = if mode == LockMode::Encrypted {
                card_keys
            } else {
                &[]
            };
            crate::shares::replace_card_keys(conn, &id.to_string(), keys)?;
            if was_encrypted && mode != LockMode::Encrypted {
                // Without card keys these snapshots are unreadable ciphertext.
                diesel::delete(
                    card_revisions::table
                        .filter(card_revisions::card_id.eq(id.to_string()))
                        .filter(card_revisions::encrypted.is_not_null()),
                )
                .execute(conn)?;
            }
            // Old plaintext revisions and images must not survive encryption.
            if mode == LockMode::Encrypted {
                diesel::delete(
                    card_revisions::table.filter(card_revisions::card_id.eq(id.to_string())),
                )
                .execute(conn)?;
                let keep: Vec<&str> = blob_ids
                    .iter()
                    .map(|r| cardfile_core::BlobRef::parse(r).id)
                    .collect();
                crate::blobs::unlink_card_blobs(conn, &id.to_string(), &keep)?;
            }
            add_card_blobs(conn, id, blob_ids)?;
            let title = title_of(conn, &id.to_string())?;
            crate::activity::log(conn, actor, Some(&id.to_string()), mode.as_str(), &title)?;
            load_card(conn, id)
        })?;
        if mode == LockMode::Encrypted {
            // The encryption is committed; failing to checkpoint must not undo it.
            if let Err(e) = self.with(checkpoint_wal) {
                tracing::warn!("WAL checkpoint after encrypting a card failed: {e}");
            }
        }
        Ok(card)
    }

    pub fn set_card_pinned(&self, actor: &Actor, id: CardId, pinned: bool) -> DbResult<()> {
        let action = if pinned { "pin" } else { "unpin" };
        self.touch(actor, id, action, true, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::is_pinned.eq(pinned))
                .execute(conn)
        })
    }

    pub fn set_card_favorite(&self, actor: &Actor, id: CardId, favorite: bool) -> DbResult<()> {
        let action = if favorite { "favorite" } else { "unfavorite" };
        self.touch(actor, id, action, true, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::is_favorite.eq(favorite))
                .execute(conn)
        })
    }

    /// Move a card between two neighbours in the manual order (`None` = list edge).
    pub fn move_card(
        &self,
        actor: &Actor,
        id: CardId,
        before: Option<CardId>,
        after: Option<CardId>,
    ) -> DbResult<()> {
        self.tx(|conn| {
            ensure_owner(conn, actor, &id.to_string())?;
            let key_of =
                |conn: &mut SqliteConnection, c: Option<CardId>| -> DbResult<Option<String>> {
                    c.map(|c| {
                        cards::table
                            .find(c.to_string())
                            .select(cards::sort_key)
                            .first::<String>(conn)
                            .map_err(DbError::from)
                    })
                    .transpose()
                };
            // `before` is the card that will sit above, `after` the one below.
            let lo = key_of(conn, before)?;
            let hi = key_of(conn, after)?;
            let key = order::key_between(lo.as_deref(), hi.as_deref());
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::sort_key.eq(key),
                    cards::updated_at.eq(now_millis()),
                    cards::updated_by.eq(actor.user.to_string()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            let title = title_of(conn, &id.to_string())?;
            crate::activity::log(conn, actor, Some(&id.to_string()), "move", &title)?;
            Ok(())
        })
    }

    /// Move a card to the recycle bin. Owner only.
    pub fn delete_card(&self, actor: &Actor, id: CardId) -> DbResult<()> {
        let now = now_millis();
        self.touch(actor, id, "delete", true, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::deleted_at.eq(now))
                .execute(conn)
        })
    }

    pub fn restore_card(&self, actor: &Actor, id: CardId) -> DbResult<()> {
        self.touch(actor, id, "restore", true, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::deleted_at.eq(None::<i64>))
                .execute(conn)
        })
    }

    /// Permanently remove one card from the recycle bin (a card that is not in the
    /// bin is left alone). Owner only. This and [`Self::empty_recycle_bin`] are the
    /// only hard deletes of cards.
    pub fn purge_card(&self, actor: &Actor, id: CardId) -> DbResult<()> {
        self.tx(|conn| {
            ensure_owner(conn, actor, &id.to_string())?;
            let n = diesel::delete(
                cards::table
                    .find(id.to_string())
                    .filter(cards::deleted_at.is_not_null()),
            )
            .execute(conn)?;
            if n > 0 {
                scrub_activity(conn, &[id.to_string()])?;
            }
            crate::blobs::gc(conn)?;
            Ok(())
        })
    }

    /// Permanently remove every card in the owner's recycle bin.
    pub fn empty_recycle_bin(&self, owner: UserId) -> DbResult<usize> {
        self.tx(|conn| {
            let binned = || {
                cards::table
                    .filter(cards::owner_id.eq(owner.to_string()))
                    .filter(cards::deleted_at.is_not_null())
            };
            let ids: Vec<String> = binned().select(cards::id).load(conn)?;
            let n = diesel::delete(binned()).execute(conn)?;
            scrub_activity(conn, &ids)?;
            crate::blobs::gc(conn)?;
            Ok(n)
        })
    }

    pub fn card_revisions(&self, id: CardId) -> DbResult<Vec<CardRevision>> {
        self.with(|conn| {
            card_revisions::table
                .filter(card_revisions::card_id.eq(id.to_string()))
                .order(card_revisions::created_at.desc())
                .select(RevisionRow::as_select())
                .load(conn)?
                .into_iter()
                .map(TryInto::try_into)
                .collect()
        })
    }

    /// Delete all revisions of a card. Owner only.
    ///
    /// Revisions are local history, not synced entities, so they are hard-deleted
    /// (like pruned snapshots); clearing history is exactly what the user asks for.
    pub fn clear_card_revisions(&self, actor: &Actor, id: CardId) -> DbResult<()> {
        self.tx(|conn| {
            ensure_owner(conn, actor, &id.to_string())?;
            diesel::delete(
                card_revisions::table.filter(card_revisions::card_id.eq(id.to_string())),
            )
            .execute(conn)?;
            Ok(())
        })
    }

    pub fn card_revision(&self, id: RevisionId) -> DbResult<CardRevision> {
        self.with(|conn| {
            card_revisions::table
                .find(id.to_string())
                .select(RevisionRow::as_select())
                .first(conn)?
                .try_into()
        })
    }

    /// Bump `updated_at`/`rev` around a small update, check permissions and log it.
    fn touch(
        &self,
        actor: &Actor,
        id: CardId,
        action: &str,
        owner_only: bool,
        f: impl FnOnce(&mut SqliteConnection, String) -> QueryResult<usize>,
    ) -> DbResult<()> {
        self.tx(|conn| {
            if owner_only {
                ensure_owner(conn, actor, &id.to_string())?;
            } else {
                ensure_can_edit(conn, actor, &id.to_string())?;
            }
            let n = f(conn, id.to_string())?;
            if n == 0 {
                return Err(DbError::Query(diesel::result::Error::NotFound));
            }
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::updated_at.eq(now_millis()),
                    cards::updated_by.eq(actor.user.to_string()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            let title = title_of(conn, &id.to_string())?;
            crate::activity::log(conn, actor, Some(&id.to_string()), action, &title)
        })
    }
}

pub(crate) fn load_card(conn: &mut SqliteConnection, id: CardId) -> DbResult<Card> {
    let row: CardRow = cards::table
        .find(id.to_string())
        .select(CardRow::as_select())
        .first(conn)?;
    let tags = tags_of(conn, &[id.to_string()])?
        .remove(&id.to_string())
        .unwrap_or_default();
    row.into_card(tags)
}

/// Tag ids per card id.
pub(crate) fn tags_of(
    conn: &mut SqliteConnection,
    card_ids: &[String],
) -> DbResult<HashMap<String, Vec<TagId>>> {
    let mut map: HashMap<String, Vec<TagId>> = HashMap::new();
    for chunk in card_ids.chunks(500) {
        let pairs: Vec<(String, String)> = card_tags::table
            .filter(card_tags::card_id.eq_any(chunk))
            .filter(card_tags::deleted_at.is_null())
            .select((card_tags::card_id, card_tags::tag_id))
            .load(conn)?;
        for (card, tag) in pairs {
            map.entry(card).or_default().push(parse_id(&tag)?);
        }
    }
    Ok(map)
}

fn insert_revision(conn: &mut SqliteConnection, old: &CardRow, now: i64) -> DbResult<()> {
    let row = RevisionRow {
        id: RevisionId::new().to_string(),
        card_id: old.id.clone(),
        title: old.title.clone(),
        content: old.content.clone(),
        encrypted: old.encrypted.clone(),
        created_at: now,
        author_id: old.updated_by.clone(),
    };
    diesel::insert_into(card_revisions::table)
        .values(&row)
        .execute(conn)?;
    Ok(())
}

fn prune_revisions(conn: &mut SqliteConnection, id: CardId, keep: u32) -> DbResult<()> {
    let stale: Vec<String> = card_revisions::table
        .filter(card_revisions::card_id.eq(id.to_string()))
        .order(card_revisions::created_at.desc())
        .offset(i64::from(keep))
        .limit(i64::MAX)
        .select(card_revisions::id)
        .load(conn)?;
    if !stale.is_empty() {
        diesel::delete(card_revisions::table.filter(card_revisions::id.eq_any(stale)))
            .execute(conn)?;
    }
    Ok(())
}

/// Write any WAL content back into the database file and truncate the WAL, so
/// pages that held replaced plaintext do not stay readable in it. A no-op for
/// in-memory databases.
fn checkpoint_wal(conn: &mut SqliteConnection) -> DbResult<()> {
    use diesel::connection::SimpleConnection;
    conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

/// Link blobs to a card. Links are only ever added: older revisions may still show a
/// blob the current content dropped, so links go away only when the card is purged.
///
/// References to blobs that do not exist (e.g. collected by GC while an editor still
/// showed them) are skipped: linking them would violate the foreign key and make
/// every later save of the card fail.
fn add_card_blobs(conn: &mut SqliteConnection, id: CardId, blob_ids: &[String]) -> DbResult<()> {
    // Documents hold references; sealed ones carry their key after the id.
    let mut ids: Vec<&str> = blob_ids
        .iter()
        .map(|r| cardfile_core::BlobRef::parse(r).id)
        .collect();
    ids.sort();
    ids.dedup();
    let mut existing: Vec<String> = Vec::new();
    for chunk in ids.chunks(500) {
        existing.extend(
            blobs::table
                .filter(blobs::id.eq_any(chunk))
                .select(blobs::id)
                .load::<String>(conn)?,
        );
    }
    let rows: Vec<_> = existing
        .into_iter()
        .map(|b| {
            (
                card_blobs::card_id.eq(id.to_string()),
                card_blobs::blob_id.eq(b),
            )
        })
        .collect();
    if !rows.is_empty() {
        diesel::insert_or_ignore_into(card_blobs::table)
            .values(rows)
            .execute(conn)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use cardfile_core::{CardFilter, LockMode, NewCard};

    use super::*;
    use crate::test_util::*;

    fn new_card(title: &str) -> NewCard {
        NewCard {
            title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn create_update_and_history() {
        let (db, user) = db_with_user();
        let card = db
            .create_card(&Actor::user(user.id), new_card("First"))
            .unwrap();
        assert_eq!(card.rev, 1);
        let upd = |t: &str| ContentUpdate {
            title: t.into(),
            content: format!("{{\"t\":\"{t}\"}}"),
            ..Default::default()
        };
        let card = db
            .update_card_content(&Actor::user(user.id), card.id, upd("Second"), 3, 0)
            .unwrap();
        assert_eq!(card.title, "Second");
        assert_eq!(card.rev, 2);
        for i in 0..5 {
            db.update_card_content(&Actor::user(user.id), card.id, upd(&format!("v{i}")), 3, 0)
                .unwrap();
        }
        let revs = db.card_revisions(card.id).unwrap();
        assert_eq!(revs.len(), 3);
        // Throttled snapshots: a huge interval adds nothing new.
        db.update_card_content(&Actor::user(user.id), card.id, upd("again"), 3, i64::MAX)
            .unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 3);
        let rev = db.card_revision(revs[0].id).unwrap();
        assert_eq!(rev.title, revs[0].title);
    }

    #[test]
    fn newest_cards_first_and_manual_move() {
        let (db, user) = db_with_user();
        let a = db
            .create_card(&Actor::user(user.id), new_card("a"))
            .unwrap();
        let b = db
            .create_card(&Actor::user(user.id), new_card("b"))
            .unwrap();
        let c = db
            .create_card(&Actor::user(user.id), new_card("c"))
            .unwrap();
        let titles = |db: &Database| -> Vec<String> {
            db.list_cards(user.id, &CardFilter::default())
                .unwrap()
                .into_iter()
                .map(|c| c.title)
                .collect()
        };
        assert_eq!(titles(&db), ["c", "b", "a"]);
        // Move `a` between `c` and `b`.
        db.move_card(&Actor::user(user.id), a.id, Some(c.id), Some(b.id))
            .unwrap();
        assert_eq!(titles(&db), ["c", "a", "b"]);
        db.move_card(&Actor::user(user.id), c.id, Some(b.id), None)
            .unwrap();
        assert_eq!(titles(&db), ["a", "b", "c"]);
    }

    #[test]
    fn recycle_bin() {
        let (db, user) = db_with_user();
        let a = db
            .create_card(&Actor::user(user.id), new_card("a"))
            .unwrap();
        let b = db
            .create_card(&Actor::user(user.id), new_card("b"))
            .unwrap();
        db.delete_card(&Actor::user(user.id), a.id).unwrap();
        db.delete_card(&Actor::user(user.id), b.id).unwrap();
        let bin = CardFilter {
            deleted: true,
            ..Default::default()
        };
        assert_eq!(db.list_cards(user.id, &bin).unwrap().len(), 2);
        db.restore_card(&Actor::user(user.id), a.id).unwrap();
        assert_eq!(
            db.list_cards(user.id, &CardFilter::default())
                .unwrap()
                .len(),
            1
        );
        // Purge only works on deleted cards.
        db.purge_card(&Actor::user(user.id), a.id).unwrap();
        assert!(db.card(a.id).is_ok());
        assert_eq!(db.empty_recycle_bin(user.id).unwrap(), 1);
        assert!(db.card(b.id).unwrap_err().is_not_found());
    }

    #[test]
    fn encryption_drops_plain_revisions() {
        let (db, user) = db_with_user();
        let card = db
            .create_card(&Actor::user(user.id), new_card("secret"))
            .unwrap();
        db.update_card_content(
            &Actor::user(user.id),
            card.id,
            ContentUpdate {
                title: "s2".into(),
                content: "x".into(),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 1);
        let payload = EncryptedPayload {
            v: 1,
            salt: "s".into(),
            nonce: "n".into(),
            m_cost: 1,
            t_cost: 1,
            p_cost: 1,
            ciphertext: "c".into(),
        };
        assert!(
            db.set_card_lock(
                &Actor::user(user.id),
                card.id,
                LockMode::Encrypted,
                "",
                "",
                None,
                &[],
                &[]
            )
            .is_err()
        );
        let card = db
            .set_card_lock(
                &Actor::user(user.id),
                card.id,
                LockMode::Encrypted,
                "",
                "",
                Some(&payload),
                &[],
                &[],
            )
            .unwrap();
        assert_eq!(card.lock_mode, LockMode::Encrypted);
        assert_eq!(card.encrypted, Some(payload));
        assert!(db.card_revisions(card.id).unwrap().is_empty());
    }

    #[test]
    fn pin_and_favorite() {
        let (db, user) = db_with_user();
        let a = db
            .create_card(&Actor::user(user.id), new_card("a"))
            .unwrap();
        db.set_card_pinned(&Actor::user(user.id), a.id, true)
            .unwrap();
        db.set_card_favorite(&Actor::user(user.id), a.id, true)
            .unwrap();
        let a = db.card(a.id).unwrap();
        assert!(a.is_pinned && a.is_favorite);
        assert_eq!(a.rev, 3);
        assert!(
            db.set_card_pinned(&Actor::user(user.id), CardId::new(), true)
                .unwrap_err()
                .is_not_found()
        );
    }

    #[test]
    fn blobs_dropped_from_content_survive_for_history() {
        let (db, user) = db_with_user();
        let keep = db
            .create_card(&Actor::user(user.id), new_card("keep"))
            .unwrap();
        let gone = db
            .create_card(&Actor::user(user.id), new_card("gone"))
            .unwrap();
        let blob = db.put_blob(b"img", "image/png", None).unwrap();
        let with_blob = ContentUpdate {
            title: "keep".into(),
            blob_ids: vec![blob.id.0.clone()],
            ..Default::default()
        };
        db.update_card_content(&Actor::user(user.id), keep.id, with_blob, 10, 0)
            .unwrap();
        // The image is removed again; the previous revision still shows it.
        let without = ContentUpdate {
            title: "keep 2".into(),
            ..Default::default()
        };
        db.update_card_content(&Actor::user(user.id), keep.id, without, 10, 0)
            .unwrap();
        // Age the blob past the grace period, then purge an unrelated card (runs GC).
        db.with(|conn| {
            diesel::sql_query("UPDATE blobs SET created_at = 0").execute(conn)?;
            Ok(())
        })
        .unwrap();
        db.delete_card(&Actor::user(user.id), gone.id).unwrap();
        db.purge_card(&Actor::user(user.id), gone.id).unwrap();
        assert!(db.blob(&blob.id.0).unwrap().is_some());
        // Once the card itself is purged the blob goes too.
        db.delete_card(&Actor::user(user.id), keep.id).unwrap();
        db.purge_card(&Actor::user(user.id), keep.id).unwrap();
        assert!(db.blob(&blob.id.0).unwrap().is_none());
    }

    #[test]
    fn move_card_bumps_updated_at() {
        let (db, user) = db_with_user();
        let a = db
            .create_card(&Actor::user(user.id), new_card("a"))
            .unwrap();
        let b = db
            .create_card(&Actor::user(user.id), new_card("b"))
            .unwrap();
        db.with(|conn| {
            diesel::sql_query("UPDATE cards SET updated_at = 0").execute(conn)?;
            Ok(())
        })
        .unwrap();
        db.move_card(&Actor::user(user.id), a.id, Some(b.id), None)
            .unwrap();
        let a = db.card(a.id).unwrap();
        assert!(a.updated_at > 0);
        assert_eq!(a.rev, 2);
    }

    #[test]
    fn encrypting_a_card_drops_its_plaintext_blobs() {
        let (db, user) = db_with_user();
        let card = db
            .create_card(&Actor::user(user.id), new_card("secret"))
            .unwrap();
        let other = db
            .create_card(&Actor::user(user.id), new_card("other"))
            .unwrap();
        let mine = db.put_blob(b"mine", "image/png", None).unwrap().id.0;
        let shared = db.put_blob(b"shared", "image/png", None).unwrap().id.0;
        let link = |id, blobs: Vec<String>| {
            let upd = ContentUpdate {
                title: "t".into(),
                blob_ids: blobs,
                ..Default::default()
            };
            db.update_card_content(&Actor::user(user.id), id, upd, 10, 0)
                .unwrap();
        };
        link(card.id, vec![mine.clone(), shared.clone()]);
        link(other.id, vec![shared.clone()]);
        db.put_sealed_blob("s1", b"nonce+ciphertext").unwrap();
        let sealed = cardfile_core::BlobRef::sealed("s1", "key");
        let payload = EncryptedPayload {
            v: 1,
            salt: "s".into(),
            nonce: "n".into(),
            m_cost: 1,
            t_cost: 1,
            p_cost: 1,
            ciphertext: "c".into(),
        };
        db.set_card_lock(
            &Actor::user(user.id),
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload),
            &[],
            &[sealed],
        )
        .unwrap();
        // Plaintext only this card used is gone; the shared one and the sealed one stay.
        assert!(db.blob(&mine).unwrap().is_none());
        assert!(db.blob(&shared).unwrap().is_some());
        let s1 = db.blob("s1").unwrap().unwrap();
        assert_eq!(s1.meta.mime, "application/octet-stream");
        assert_eq!(s1.meta.file_name, None);
        // The sealed blob is linked by its id, so GC keeps it.
        db.with(|conn| {
            diesel::sql_query("UPDATE blobs SET created_at = 0").execute(conn)?;
            Ok(())
        })
        .unwrap();
        db.collect_garbage_blobs().unwrap();
        assert!(db.blob("s1").unwrap().is_some());
        // Decrypt and encrypt again: the sealed blob is still in use and must stay.
        let sealed = cardfile_core::BlobRef::sealed("s1", "key");
        db.set_card_lock(
            &Actor::user(user.id),
            card.id,
            LockMode::None,
            "{}",
            "",
            None,
            &[],
            std::slice::from_ref(&sealed),
        )
        .unwrap();
        db.set_card_lock(
            &Actor::user(user.id),
            card.id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload),
            &[],
            &[sealed],
        )
        .unwrap();
        assert!(db.blob("s1").unwrap().is_some());
    }

    fn payload(v: u32) -> EncryptedPayload {
        EncryptedPayload {
            v,
            salt: String::new(),
            nonce: "n".into(),
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
            ciphertext: "c".into(),
        }
    }

    fn encrypt(db: &Database, actor: &Actor, id: CardId) -> Card {
        db.set_card_lock(
            actor,
            id,
            LockMode::Encrypted,
            "",
            "",
            Some(&payload(2)),
            &[(actor.principal(), "wrapped".into())],
            &[],
        )
        .unwrap()
    }

    fn cipher_update(title: &str) -> ContentUpdate {
        ContentUpdate {
            title: title.into(),
            encrypted: Some(payload(2)),
            ..Default::default()
        }
    }

    #[test]
    fn decrypting_drops_the_encrypted_history() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        let card = db.create_card(&me, new_card("secret")).unwrap();
        encrypt(&db, &me, card.id);
        db.update_card_content(&me, card.id, cipher_update("s2"), 10, 0)
            .unwrap();
        db.update_card_content(&me, card.id, cipher_update("s3"), 10, 0)
            .unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 2);
        let card = db
            .set_card_lock(&me, card.id, LockMode::None, "{}", "plain", None, &[], &[])
            .unwrap();
        assert_eq!(card.lock_mode, LockMode::None);
        assert_eq!(db.wrapped_card_key(card.id, user.id).unwrap(), None);
        // No ciphertext is left without a key to open it.
        assert!(
            db.card_revisions(card.id)
                .unwrap()
                .iter()
                .all(|r| r.encrypted.is_none())
        );
        // Plain history starts again from here.
        let upd = ContentUpdate {
            title: "plain 2".into(),
            content: "{\"x\":1}".into(),
            ..Default::default()
        };
        db.update_card_content(&me, card.id, upd, 10, 0).unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 1);
    }

    #[test]
    fn saves_must_match_the_current_lock_mode() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        let card = db.create_card(&me, new_card("c")).unwrap();
        // Ciphertext into a plain card is refused.
        let err = db
            .update_card_content(&me, card.id, cipher_update("x"), 10, 0)
            .unwrap_err();
        assert!(matches!(
            err,
            DbError::Domain(cardfile_core::Error::LockModeMismatch)
        ));
        encrypt(&db, &me, card.id);
        // A plaintext save prepared before the card was encrypted is refused.
        let stale = ContentUpdate {
            title: "c".into(),
            content: "{\"plain\":true}".into(),
            plain_text: "plain".into(),
            ..Default::default()
        };
        let err = db
            .update_card_content(&me, card.id, stale, 10, 0)
            .unwrap_err();
        assert!(matches!(
            err,
            DbError::Domain(cardfile_core::Error::LockModeMismatch)
        ));
        let card = db.card(card.id).unwrap();
        assert!(card.content.is_empty() && card.plain_text.is_empty());
        db.update_card_content(&me, card.id, cipher_update("ok"), 10, 0)
            .unwrap();
    }

    #[test]
    fn missing_blobs_are_not_linked() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        let card = db.create_card(&me, new_card("c")).unwrap();
        let blob = db.put_blob(b"img", "image/png", None).unwrap().id.0;
        let upd = ContentUpdate {
            title: "c".into(),
            blob_ids: vec![blob.clone(), "gone-blob".into()],
            ..Default::default()
        };
        db.update_card_content(&me, card.id, upd.clone(), 10, 0)
            .unwrap();
        db.update_card_content(&me, card.id, upd, 10, 0).unwrap();
        let linked: Vec<String> = db
            .with(|conn| Ok(card_blobs::table.select(card_blobs::blob_id).load(conn)?))
            .unwrap();
        assert_eq!(linked, [blob]);
    }

    #[test]
    fn only_the_owner_purges_and_clears_history() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let b = Actor::user(bob.id);
        let card = db.create_card(&a, new_card("c")).unwrap();
        db.share_card(
            &a,
            card.id,
            bob.id,
            cardfile_core::SharePermission::Edit,
            None,
        )
        .unwrap();
        let upd = ContentUpdate {
            title: "c2".into(),
            content: "x".into(),
            ..Default::default()
        };
        db.update_card_content(&b, card.id, upd, 10, 0).unwrap();
        assert!(db.clear_card_revisions(&b, card.id).is_err());
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 1);
        db.clear_card_revisions(&a, card.id).unwrap();
        assert!(db.card_revisions(card.id).unwrap().is_empty());
        db.delete_card(&a, card.id).unwrap();
        assert!(db.purge_card(&b, card.id).is_err());
        assert!(db.card(card.id).is_ok());
        db.purge_card(&a, card.id).unwrap();
        assert!(db.card(card.id).unwrap_err().is_not_found());
    }

    #[test]
    fn purged_cards_leave_no_titles_in_the_log() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        let one = db.create_card(&me, new_card("Secret one")).unwrap();
        let two = db.create_card(&me, new_card("Secret two")).unwrap();
        let kept = db.create_card(&me, new_card("Kept")).unwrap();
        db.delete_card(&me, one.id).unwrap();
        db.delete_card(&me, two.id).unwrap();
        db.purge_card(&me, one.id).unwrap();
        assert_eq!(db.empty_recycle_bin(user.id).unwrap(), 1);
        let log = db.activity(None, None, 100).unwrap();
        assert!(log.iter().all(|e| !e.summary.contains("Secret")), "{log:?}");
        assert!(
            log.iter()
                .any(|e| e.card_id == Some(kept.id) && e.summary == "Kept")
        );
    }

    #[test]
    fn prepending_many_cards_keeps_sort_keys_short() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        for _ in 0..300 {
            db.create_card(&me, new_card("c")).unwrap();
        }
        let list = db.list_cards(user.id, &CardFilter::default()).unwrap();
        assert!(list.iter().all(|c| c.sort_key.len() <= 3), "{list:?}");
        assert!(list.windows(2).all(|w| w[0].sort_key < w[1].sort_key));
    }

    #[test]
    fn encrypted_plaintext_is_not_left_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cardfile.db");
        let db = Database::open(&path, None).unwrap();
        let user = db.create_user("alice", "Alice", None, true).unwrap();
        let me = Actor::user(user.id);
        let marker = "qzxwvplaintextmarker";
        let card = db
            .create_card(
                &me,
                NewCard {
                    title: "t".into(),
                    content: format!("{{\"text\":\"{marker}\"}}"),
                    plain_text: marker.into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let upd = ContentUpdate {
            title: "t".into(),
            content: format!("{{\"text\":\"{marker} again\"}}"),
            plain_text: format!("{marker} again"),
            ..Default::default()
        };
        db.update_card_content(&me, card.id, upd, 10, 0).unwrap();
        encrypt(&db, &me, card.id);
        let mut bytes = std::fs::read(&path).unwrap();
        let wal = dir.path().join("cardfile.db-wal");
        if wal.exists() {
            bytes.extend(std::fs::read(&wal).unwrap());
        }
        let found = bytes.windows(marker.len()).any(|w| w == marker.as_bytes());
        assert!(!found, "plaintext survived encryption on disk");
    }
}
