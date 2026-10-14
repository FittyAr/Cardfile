use cardfile_core::{CardId, Tag, TagId, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::TagRow;
use crate::schema::{card_tags, cards, tags};
use crate::{Database, DbResult};

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

    pub fn update_tag(&self, id: TagId, name: &str, color: &str) -> DbResult<()> {
        self.tx(|conn| {
            diesel::update(tags::table.find(id.to_string()))
                .set((
                    tags::name.eq(name.trim()),
                    tags::color.eq(color),
                    tags::updated_at.eq(now_millis()),
                    tags::rev.eq(tags::rev + 1),
                ))
                .execute(conn)?;
            let cards: Vec<String> = card_tags::table
                .filter(card_tags::tag_id.eq(id.to_string()))
                .select(card_tags::card_id)
                .load(conn)?;
            for card in cards {
                refresh_fts_tags(conn, &card)?;
            }
            Ok(())
        })
    }

    /// Soft-delete a tag and detach it from every card.
    pub fn delete_tag(&self, id: TagId) -> DbResult<()> {
        self.tx(|conn| {
            let cards: Vec<String> = card_tags::table
                .filter(card_tags::tag_id.eq(id.to_string()))
                .select(card_tags::card_id)
                .load(conn)?;
            diesel::delete(card_tags::table.filter(card_tags::tag_id.eq(id.to_string())))
                .execute(conn)?;
            let now = now_millis();
            diesel::update(tags::table.find(id.to_string()))
                .set((
                    tags::deleted_at.eq(now),
                    tags::updated_at.eq(now),
                    tags::rev.eq(tags::rev + 1),
                ))
                .execute(conn)?;
            // Losing a tag is a change to each card, so sync must see it.
            diesel::update(cards::table.filter(cards::id.eq_any(&cards)))
                .set((cards::updated_at.eq(now), cards::rev.eq(cards::rev + 1)))
                .execute(conn)?;
            for card in cards {
                refresh_fts_tags(conn, &card)?;
            }
            Ok(())
        })
    }

    /// Replace the tags of a card.
    pub fn set_card_tags(&self, card: CardId, tag_ids: &[TagId]) -> DbResult<()> {
        let card_id = card.to_string();
        self.tx(|conn| {
            diesel::delete(card_tags::table.filter(card_tags::card_id.eq(&card_id)))
                .execute(conn)?;
            let rows: Vec<_> = tag_ids
                .iter()
                .map(|t| {
                    (
                        card_tags::card_id.eq(&card_id),
                        card_tags::tag_id.eq(t.to_string()),
                    )
                })
                .collect();
            if !rows.is_empty() {
                diesel::insert_or_ignore_into(card_tags::table)
                    .values(rows)
                    .execute(conn)?;
            }
            diesel::update(cards::table.find(&card_id))
                .set((
                    cards::updated_at.eq(now_millis()),
                    cards::rev.eq(cards::rev + 1),
                ))
                .execute(conn)?;
            refresh_fts_tags(conn, &card_id)
        })
    }
}

/// Copy a card's tag names into the FTS index.
fn refresh_fts_tags(conn: &mut SqliteConnection, card_id: &str) -> DbResult<()> {
    diesel::sql_query(
        "UPDATE cards_fts SET tags = COALESCE((
             SELECT group_concat(t.name, ' ') FROM card_tags ct
             JOIN tags t ON t.id = ct.tag_id AND t.deleted_at IS NULL
             WHERE ct.card_id = ?1), '')
         WHERE card_id = ?1",
    )
    .bind::<diesel::sql_types::Text, _>(card_id)
    .execute(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cardfile_core::{CardFilter, NewCard, SearchScope};

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
                user.id,
                NewCard {
                    title: "a".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let b = db
            .create_card(
                user.id,
                NewCard {
                    title: "b".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        db.set_card_tags(a.id, &[work.id, home.id]).unwrap();
        db.set_card_tags(b.id, &[home.id]).unwrap();
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
        db.update_tag(work.id, "Office", "#ff0000").unwrap();
        assert!(db.list_cards(user.id, &search).unwrap().is_empty());

        db.delete_tag(home.id).unwrap();
        assert_eq!(db.list_tags(user.id).unwrap().len(), 1);
        assert!(db.card(b.id).unwrap().tags.is_empty());
    }
}
