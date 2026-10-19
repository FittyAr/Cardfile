//! User-defined tags and their links to cards.
//!
//! Links (`card_tags`) are synced rows: removing a tag from a card soft-deletes the
//! link (`deleted_at`, `rev`) and adding it again revives it.

use std::collections::{HashMap, HashSet};

use cardfile_core::{Actor, CardId, Tag, TagId, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::TagRow;
use crate::schema::{card_tags, cards, tags};
use crate::{Database, DbError, DbResult};

/// Fail with "not found" unless `id` is a live tag of `owner`.
fn ensure_own_tag(conn: &mut SqliteConnection, owner: UserId, id: TagId) -> DbResult<()> {
    let found: Option<String> = tags::table
        .find(id.to_string())
        .filter(tags::owner_id.eq(owner.to_string()))
        .filter(tags::deleted_at.is_null())
        .select(tags::id)
        .first(conn)
        .optional()?;
    match found {
        Some(_) => Ok(()),
        None => Err(cardfile_core::Error::NotFound(format!("tag {id}")).into()),
    }
}

/// Cards that currently carry tag `id`.
fn cards_with_tag(conn: &mut SqliteConnection, id: TagId) -> DbResult<Vec<String>> {
    Ok(card_tags::table
        .filter(card_tags::tag_id.eq(id.to_string()))
        .filter(card_tags::deleted_at.is_null())
        .select(card_tags::card_id)
        .load(conn)?)
}

impl Database {
    pub fn create_tag(&self, owner: UserId, name: &str, color: &str) -> DbResult<Tag> {
        let now = now_millis();
        let row = TagRow {
            id: TagId::new().to_string(),
            owner_id: owner.to_string(),
            name: name.trim().to_owned(),
            color: color.to_owned(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            rev: 1,
        };
        self.with(|conn| {
            let row: TagRow = diesel::insert_into(tags::table)
                .values(&row)
                .returning(TagRow::as_returning())
                .get_result(conn)?;
            row.try_into()
        })
    }

    pub fn list_tags(&self, owner: UserId) -> DbResult<Vec<Tag>> {
        self.with(|conn| {
            tags::table
                .filter(tags::owner_id.eq(owner.to_string()))
                .filter(tags::deleted_at.is_null())
                .order(tags::name.asc())
                .select(TagRow::as_select())
                .load(conn)?
                .into_iter()
                .map(TryInto::try_into)
                .collect()
        })
    }

    /// Rename or recolour one of `owner`'s live tags.
    pub fn update_tag(&self, owner: UserId, id: TagId, name: &str, color: &str) -> DbResult<()> {
        self.tx(|conn| {
            ensure_own_tag(conn, owner, id)?;
            diesel::update(tags::table.find(id.to_string()))
                .set((
                    tags::name.eq(name.trim()),
                    tags::color.eq(color),
                    tags::updated_at.eq(now_millis()),
                    tags::rev.eq(tags::rev + 1),
                ))
                .execute(conn)?;
            for card in cards_with_tag(conn, id)? {
                refresh_fts_tags(conn, &card)?;
            }
            Ok(())
        })
    }

    /// Soft-delete one of `owner`'s tags and detach it from every card.
    pub fn delete_tag(&self, owner: UserId, id: TagId) -> DbResult<()> {
        self.tx(|conn| {
            ensure_own_tag(conn, owner, id)?;
            let cards = cards_with_tag(conn, id)?;
            let now = now_millis();
            diesel::update(
                card_tags::table
                    .filter(card_tags::tag_id.eq(id.to_string()))
                    .filter(card_tags::deleted_at.is_null()),
            )
            .set((
                card_tags::deleted_at.eq(now),
                card_tags::updated_at.eq(now),
                card_tags::rev.eq(card_tags::rev + 1),
            ))
            .execute(conn)?;
            diesel::update(tags::table.find(id.to_string()))
                .set((
                    tags::deleted_at.eq(now),
                    tags::updated_at.eq(now),
                    tags::rev.eq(tags::rev + 1),
                ))
                .execute(conn)?;
            // Losing a tag is a change to each card, so sync must see it.
            for chunk in cards.chunks(500) {
                diesel::update(cards::table.filter(cards::id.eq_any(chunk)))
                    .set((cards::updated_at.eq(now), cards::rev.eq(cards::rev + 1)))
                    .execute(conn)?;
            }
            for card in cards {
                refresh_fts_tags(conn, &card)?;
            }
            Ok(())
        })
    }

    /// Replace the tags of a card. Tags belong to the card's owner, so only they may,
    /// and only with their own live tags.
    pub fn set_card_tags(&self, actor: &Actor, card: CardId, tag_ids: &[TagId]) -> DbResult<()> {
        let card_id = card.to_string();
        let wanted: HashSet<String> = tag_ids.iter().map(ToString::to_string).collect();
        self.tx(|conn| {
            crate::cards::ensure_owner(conn, actor, &card_id)?;
            let owner = actor.principal().to_string();
            let valid: HashSet<String> = tags::table
                .filter(tags::id.eq_any(&wanted))
                .filter(tags::owner_id.eq(&owner))
                .filter(tags::deleted_at.is_null())
                .select(tags::id)
                .load::<String>(conn)?
                .into_iter()
                .collect();
            if let Some(bad) = wanted.iter().find(|t| !valid.contains(*t)) {
                return Err(DbError::Domain(cardfile_core::Error::NotFound(format!(
                    "tag {bad}"
                ))));
            }
            // Current links, live or not: `true` when soft-deleted.
            let current: HashMap<String, bool> = card_tags::table
                .filter(card_tags::card_id.eq(&card_id))
                .select((card_tags::tag_id, card_tags::deleted_at))
                .load::<(String, Option<i64>)>(conn)?
                .into_iter()
                .map(|(tag, deleted)| (tag, deleted.is_some()))
                .collect();
            let now = now_millis();
            let dropped: Vec<&String> = current
                .iter()
                .filter(|(tag, deleted)| !**deleted && !wanted.contains(*tag))
                .map(|(tag, _)| tag)
                .collect();
            let revived: Vec<&String> = wanted
                .iter()
                .filter(|t| current.get(*t) == Some(&true))
                .collect();
            diesel::update(
                card_tags::table
                    .filter(card_tags::card_id.eq(&card_id))
                    .filter(card_tags::tag_id.eq_any(&dropped)),
            )
            .set((
                card_tags::deleted_at.eq(now),
                card_tags::updated_at.eq(now),
                card_tags::rev.eq(card_tags::rev + 1),
            ))
            .execute(conn)?;
            diesel::update(
                card_tags::table
                    .filter(card_tags::card_id.eq(&card_id))
                    .filter(card_tags::tag_id.eq_any(&revived)),
            )
            .set((
                card_tags::deleted_at.eq(None::<i64>),
                card_tags::updated_at.eq(now),
                card_tags::rev.eq(card_tags::rev + 1),
            ))
            .execute(conn)?;
            let added: Vec<_> = wanted
                .iter()
                .filter(|t| !current.contains_key(*t))
                .map(|t| {
                    (
                        card_tags::card_id.eq(&card_id),
                        card_tags::tag_id.eq(t),
                        card_tags::created_at.eq(now),
                        card_tags::updated_at.eq(now),
                        card_tags::rev.eq(1),
                    )
                })
                .collect();
            if !added.is_empty() {
                diesel::insert_into(card_tags::table)
                    .values(added)
                    .execute(conn)?;
            }
            diesel::update(cards::table.find(&card_id))
                .set((
                    cards::updated_at.eq(now),
                    cards::updated_by.eq(actor.user.to_string()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            refresh_fts_tags(conn, &card_id)?;
            let title: String = cards::table
                .find(&card_id)
                .select(cards::title)
                .first(conn)?;
            crate::activity::log(conn, actor, Some(&card_id), "tag", &title)
        })
    }
}

/// Copy a card's tag names into the FTS index.
fn refresh_fts_tags(conn: &mut SqliteConnection, card_id: &str) -> DbResult<()> {
    diesel::sql_query(
        "UPDATE cards_fts SET tags = COALESCE((
             SELECT group_concat(t.name, ' ') FROM card_tags ct
             JOIN tags t ON t.id = ct.tag_id AND t.deleted_at IS NULL
             WHERE ct.card_id = ?1 AND ct.deleted_at IS NULL), '')
         WHERE card_id = ?1",
    )
    .bind::<diesel::sql_types::Text, _>(card_id)
    .execute(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cardfile_core::Actor;
    use cardfile_core::{CardFilter, NewCard, SearchScope, TagId};
    use diesel::prelude::*;

    use crate::test_util::*;

    #[test]
    fn tags_filter_and_search() {
        let (db, user) = db_with_user();
        let work = db.create_tag(user.id, "Work", "#ff0000").unwrap();
        let home = db.create_tag(user.id, "home", "#00ff00").unwrap();
        assert!(
            db.create_tag(user.id, "work", "#000000")
                .unwrap_err()
                .is_unique_violation()
        );
        let a = db
            .create_card(
                &Actor::user(user.id),
                NewCard {
                    title: "a".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let b = db
            .create_card(
                &Actor::user(user.id),
                NewCard {
                    title: "b".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        db.set_card_tags(&Actor::user(user.id), a.id, &[work.id, home.id])
            .unwrap();
        db.set_card_tags(&Actor::user(user.id), b.id, &[home.id])
            .unwrap();
        assert_eq!(db.card(a.id).unwrap().tags.len(), 2);

        let by_tag = CardFilter {
            tag: Some(work.id),
            ..Default::default()
        };
        assert_eq!(db.list_cards(user.id, &by_tag).unwrap().len(), 1);

        let search = CardFilter {
            query: "work".into(),
            scope: SearchScope::Everything,
            ..Default::default()
        };
        assert_eq!(db.list_cards(user.id, &search).unwrap().len(), 1);
        db.update_tag(user.id, work.id, "Office", "#ff0000")
            .unwrap();
        assert!(db.list_cards(user.id, &search).unwrap().is_empty());

        db.delete_tag(user.id, home.id).unwrap();
        assert_eq!(db.list_tags(user.id).unwrap().len(), 1);
        assert!(db.card(b.id).unwrap().tags.is_empty());
    }

    #[test]
    fn links_are_soft_deleted_and_revived() {
        let (db, user) = db_with_user();
        let me = Actor::user(user.id);
        let work = db.create_tag(user.id, "Work", "#ff0000").unwrap();
        let card = db.create_card(&me, NewCard::default()).unwrap();
        db.set_card_tags(&me, card.id, &[work.id]).unwrap();
        db.set_card_tags(&me, card.id, &[]).unwrap();
        assert!(db.card(card.id).unwrap().tags.is_empty());
        let by_tag = CardFilter {
            tag: Some(work.id),
            ..Default::default()
        };
        assert!(db.list_cards(user.id, &by_tag).unwrap().is_empty());
        let rows = || -> Vec<(Option<i64>, i64)> {
            db.with(|conn| {
                Ok(crate::schema::card_tags::table
                    .select((
                        crate::schema::card_tags::deleted_at,
                        crate::schema::card_tags::rev,
                    ))
                    .load(conn)?)
            })
            .unwrap()
        };
        let tombstone = rows();
        assert_eq!(tombstone.len(), 1);
        assert!(tombstone[0].0.is_some());
        db.set_card_tags(&me, card.id, &[work.id]).unwrap();
        assert_eq!(db.card(card.id).unwrap().tags, [work.id]);
        assert_eq!(rows(), [(None, 3)]);
    }

    #[test]
    fn tags_are_checked_against_their_owner() {
        let (db, alice) = db_with_user();
        let bob = db.create_user("bob", "Bob", None, false).unwrap();
        let a = Actor::user(alice.id);
        let card = db.create_card(&a, NewCard::default()).unwrap();
        let bobs = db.create_tag(bob.id, "bob's", "#000000").unwrap();
        let gone = db.create_tag(alice.id, "gone", "#000000").unwrap();
        db.delete_tag(alice.id, gone.id).unwrap();
        for tag in [bobs.id, gone.id, TagId::new()] {
            assert!(db.set_card_tags(&a, card.id, &[tag]).is_err());
        }
        assert!(db.card(card.id).unwrap().tags.is_empty());
        // Other people's (or deleted) tags cannot be renamed or deleted.
        assert!(
            db.update_tag(alice.id, bobs.id, "mine", "#fff")
                .unwrap_err()
                .is_not_found()
        );
        assert!(db.delete_tag(alice.id, bobs.id).unwrap_err().is_not_found());
        assert!(db.update_tag(alice.id, gone.id, "back", "#fff").is_err());
        assert_eq!(db.list_tags(bob.id).unwrap()[0].name, "bob's");
    }
}
