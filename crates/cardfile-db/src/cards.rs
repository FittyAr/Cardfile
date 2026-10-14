use std::collections::HashMap;

use cardfile_core::{
    Card, CardId, CardRevision, EncryptedPayload, LockMode, NewCard, RevisionId, TagId, UserId,
    now_millis, order,
};
use diesel::prelude::*;

use crate::rows::{CardRow, RevisionRow, parse_id, payload_json};
use crate::schema::{card_blobs, card_revisions, card_tags, cards};
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

impl Database {
    /// Create a card at the top of the owner's manual order.
    pub fn create_card(&self, owner: UserId, new: NewCard) -> DbResult<Card> {
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
            };
            let row: CardRow = diesel::insert_into(cards::table)
                .values(&row)
                .returning(CardRow::as_returning())
                .get_result(conn)?;
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
        id: CardId,
        update: ContentUpdate,
        keep_revisions: u32,
        snapshot_every_ms: i64,
    ) -> DbResult<Card> {
        self.tx(|conn| {
            let old: CardRow = cards::table
                .find(id.to_string())
                .select(CardRow::as_select())
                .first(conn)?;
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
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            add_card_blobs(conn, id, &update.blob_ids)?;
            load_card(conn, id)
        })
    }

    /// Change the protection level. For [`LockMode::Encrypted`] pass the ciphertext and
    /// an empty `content`; when leaving encryption pass the decrypted content.
    pub fn set_card_lock(
        &self,
        id: CardId,
        mode: LockMode,
        content: &str,
        plain_text: &str,
        encrypted: Option<&EncryptedPayload>,
    ) -> DbResult<Card> {
        if (mode == LockMode::Encrypted) != encrypted.is_some() {
            return Err(cardfile_core::Error::Invalid(
                "encrypted cards need a payload, others must not have one".into(),
            )
            .into());
        }
        self.tx(|conn| {
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::lock_mode.eq(mode.as_str()),
                    cards::content.eq(content),
                    cards::plain_text.eq(plain_text),
                    cards::encrypted.eq(payload_json(encrypted)),
                    cards::updated_at.eq(now_millis()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            // Old plaintext revisions must not survive encryption.
            if mode == LockMode::Encrypted {
                diesel::delete(
                    card_revisions::table.filter(card_revisions::card_id.eq(id.to_string())),
                )
                .execute(conn)?;
            }
            load_card(conn, id)
        })
    }

    pub fn set_card_pinned(&self, id: CardId, pinned: bool) -> DbResult<()> {
        self.touch(id, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::is_pinned.eq(pinned))
                .execute(conn)
        })
    }

    pub fn set_card_favorite(&self, id: CardId, favorite: bool) -> DbResult<()> {
        self.touch(id, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::is_favorite.eq(favorite))
                .execute(conn)
        })
    }

    /// Move a card between two neighbours in the manual order (`None` = list edge).
    pub fn move_card(
        &self,
        id: CardId,
        before: Option<CardId>,
        after: Option<CardId>,
    ) -> DbResult<()> {
        self.tx(|conn| {
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
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    /// Move a card to the recycle bin.
    pub fn delete_card(&self, id: CardId) -> DbResult<()> {
        let now = now_millis();
        self.touch(id, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::deleted_at.eq(now))
                .execute(conn)
        })
    }

    pub fn restore_card(&self, id: CardId) -> DbResult<()> {
        self.touch(id, |conn, id| {
            diesel::update(cards::table.find(id))
                .set(cards::deleted_at.eq(None::<i64>))
                .execute(conn)
        })
    }

    /// Permanently remove one card from the recycle bin.
    pub fn purge_card(&self, id: CardId) -> DbResult<()> {
        self.tx(|conn| {
            diesel::delete(
                cards::table
                    .find(id.to_string())
                    .filter(cards::deleted_at.is_not_null()),
            )
            .execute(conn)?;
            crate::blobs::gc(conn)?;
            Ok(())
        })
    }

    /// Permanently remove every card in the owner's recycle bin.
    pub fn empty_recycle_bin(&self, owner: UserId) -> DbResult<usize> {
        self.tx(|conn| {
            let n = diesel::delete(
                cards::table
                    .filter(cards::owner_id.eq(owner.to_string()))
                    .filter(cards::deleted_at.is_not_null()),
            )
            .execute(conn)?;
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

    /// Delete all revisions of a card.
    pub fn clear_card_revisions(&self, id: CardId) -> DbResult<()> {
        self.with(|conn| {
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

    /// Bump `updated_at`/`rev` around a small update.
    fn touch(
        &self,
        id: CardId,
        f: impl FnOnce(&mut SqliteConnection, String) -> QueryResult<usize>,
    ) -> DbResult<()> {
        self.tx(|conn| {
            let n = f(conn, id.to_string())?;
            if n == 0 {
                return Err(DbError::Query(diesel::result::Error::NotFound));
            }
            diesel::update(cards::table.find(id.to_string()))
                .set((
                    cards::updated_at.eq(now_millis()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
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

/// Link blobs to a card. Links are only ever added: older revisions may still show a
/// blob the current content dropped, so links go away only when the card is purged.
fn add_card_blobs(conn: &mut SqliteConnection, id: CardId, blob_ids: &[String]) -> DbResult<()> {
    let mut ids: Vec<&String> = blob_ids.iter().collect();
    ids.sort();
    ids.dedup();
    let rows: Vec<_> = ids
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
        let card = db.create_card(user.id, new_card("First")).unwrap();
        assert_eq!(card.rev, 1);
        let upd = |t: &str| ContentUpdate {
            title: t.into(),
            content: format!("{{\"t\":\"{t}\"}}"),
            ..Default::default()
        };
        let card = db
            .update_card_content(card.id, upd("Second"), 3, 0)
            .unwrap();
        assert_eq!(card.title, "Second");
        assert_eq!(card.rev, 2);
        for i in 0..5 {
            db.update_card_content(card.id, upd(&format!("v{i}")), 3, 0)
                .unwrap();
        }
        let revs = db.card_revisions(card.id).unwrap();
        assert_eq!(revs.len(), 3);
        // Throttled snapshots: a huge interval adds nothing new.
        db.update_card_content(card.id, upd("again"), 3, i64::MAX)
            .unwrap();
        assert_eq!(db.card_revisions(card.id).unwrap().len(), 3);
        let rev = db.card_revision(revs[0].id).unwrap();
        assert_eq!(rev.title, revs[0].title);
    }

    #[test]
    fn newest_cards_first_and_manual_move() {
        let (db, user) = db_with_user();
        let a = db.create_card(user.id, new_card("a")).unwrap();
        let b = db.create_card(user.id, new_card("b")).unwrap();
        let c = db.create_card(user.id, new_card("c")).unwrap();
        let titles = |db: &Database| -> Vec<String> {
            db.list_cards(user.id, &CardFilter::default())
                .unwrap()
                .into_iter()
                .map(|c| c.title)
                .collect()
        };
        assert_eq!(titles(&db), ["c", "b", "a"]);
        // Move `a` between `c` and `b`.
        db.move_card(a.id, Some(c.id), Some(b.id)).unwrap();
        assert_eq!(titles(&db), ["c", "a", "b"]);
        db.move_card(c.id, Some(b.id), None).unwrap();
        assert_eq!(titles(&db), ["a", "b", "c"]);
    }

    #[test]
    fn recycle_bin() {
        let (db, user) = db_with_user();
        let a = db.create_card(user.id, new_card("a")).unwrap();
        let b = db.create_card(user.id, new_card("b")).unwrap();
        db.delete_card(a.id).unwrap();
        db.delete_card(b.id).unwrap();
        let bin = CardFilter {
            deleted: true,
            ..Default::default()
        };
        assert_eq!(db.list_cards(user.id, &bin).unwrap().len(), 2);
        db.restore_card(a.id).unwrap();
        assert_eq!(
            db.list_cards(user.id, &CardFilter::default())
                .unwrap()
                .len(),
            1
        );
        // Purge only works on deleted cards.
        db.purge_card(a.id).unwrap();
        assert!(db.card(a.id).is_ok());
        assert_eq!(db.empty_recycle_bin(user.id).unwrap(), 1);
        assert!(db.card(b.id).unwrap_err().is_not_found());
    }

    #[test]
    fn encryption_drops_plain_revisions() {
        let (db, user) = db_with_user();
        let card = db.create_card(user.id, new_card("secret")).unwrap();
        db.update_card_content(
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
            db.set_card_lock(card.id, LockMode::Encrypted, "", "", None)
                .is_err()
        );
        let card = db
            .set_card_lock(card.id, LockMode::Encrypted, "", "", Some(&payload))
            .unwrap();
        assert_eq!(card.lock_mode, LockMode::Encrypted);
        assert_eq!(card.encrypted, Some(payload));
        assert!(db.card_revisions(card.id).unwrap().is_empty());
    }

    #[test]
    fn pin_and_favorite() {
        let (db, user) = db_with_user();
        let a = db.create_card(user.id, new_card("a")).unwrap();
        db.set_card_pinned(a.id, true).unwrap();
        db.set_card_favorite(a.id, true).unwrap();
        let a = db.card(a.id).unwrap();
        assert!(a.is_pinned && a.is_favorite);
        assert_eq!(a.rev, 3);
        assert!(
            db.set_card_pinned(CardId::new(), true)
                .unwrap_err()
                .is_not_found()
        );
    }

    #[test]
    fn blobs_dropped_from_content_survive_for_history() {
        let (db, user) = db_with_user();
        let keep = db.create_card(user.id, new_card("keep")).unwrap();
        let gone = db.create_card(user.id, new_card("gone")).unwrap();
        let blob = db.put_blob(b"img", "image/png", None).unwrap();
        let with_blob = ContentUpdate {
            title: "keep".into(),
            blob_ids: vec![blob.id.0.clone()],
            ..Default::default()
        };
        db.update_card_content(keep.id, with_blob, 10, 0).unwrap();
        // The image is removed again; the previous revision still shows it.
        let without = ContentUpdate {
            title: "keep 2".into(),
            ..Default::default()
        };
        db.update_card_content(keep.id, without, 10, 0).unwrap();
        // Age the blob past the grace period, then purge an unrelated card (runs GC).
        db.with(|conn| {
            diesel::sql_query("UPDATE blobs SET created_at = 0").execute(conn)?;
            Ok(())
        })
        .unwrap();
        db.delete_card(gone.id).unwrap();
        db.purge_card(gone.id).unwrap();
        assert!(db.blob(&blob.id.0).unwrap().is_some());
        // Once the card itself is purged the blob goes too.
        db.delete_card(keep.id).unwrap();
        db.purge_card(keep.id).unwrap();
        assert!(db.blob(&blob.id.0).unwrap().is_none());
    }

    #[test]
    fn move_card_bumps_updated_at() {
        let (db, user) = db_with_user();
        let a = db.create_card(user.id, new_card("a")).unwrap();
        let b = db.create_card(user.id, new_card("b")).unwrap();
        db.with(|conn| {
            diesel::sql_query("UPDATE cards SET updated_at = 0").execute(conn)?;
            Ok(())
        })
        .unwrap();
        db.move_card(a.id, Some(b.id), None).unwrap();
        let a = db.card(a.id).unwrap();
        assert!(a.updated_at > 0);
        assert_eq!(a.rev, 2);
    }
}
