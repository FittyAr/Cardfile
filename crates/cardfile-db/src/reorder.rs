//! Moving several cards in the manual order as one atomic change.
//!
//! [`Database::move_card`] commits each move on its own. Callers that rearrange
//! many cards at once (the AI's `reorder_cards` tool) use [`Database::move_cards`]
//! so that a failure half-way leaves the order exactly as it was, instead of a
//! partially applied arrangement.

use cardfile_core::{Actor, CardId, now_millis, order};
use diesel::prelude::*;

use crate::cards::ensure_owner;
use crate::schema::cards;
use crate::{Database, DbError, DbResult};

/// One step of [`Database::move_cards`]: put `id` between `before` (the card
/// that will sit above it) and `after` (the one below). `None` is the list edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardMove {
    /// The card to move.
    pub id: CardId,
    /// The card that ends up right above `id`.
    pub before: Option<CardId>,
    /// The card that ends up right below `id`.
    pub after: Option<CardId>,
}

impl Database {
    /// Apply `moves` in order inside a single transaction. Each move sees the sort
    /// keys written by the previous ones. Owner only, like
    /// [`Database::move_card`]; if any move fails, none is kept.
    pub fn move_cards(&self, actor: &Actor, moves: &[CardMove]) -> DbResult<()> {
        self.tx(|conn| {
            for m in moves {
                move_in(conn, actor, m)?;
            }
            Ok(())
        })
    }
}

/// The body of [`Database::move_card`], run on a connection that is already in a
/// transaction.
fn move_in(conn: &mut SqliteConnection, actor: &Actor, m: &CardMove) -> DbResult<()> {
    let id = m.id.to_string();
    ensure_owner(conn, actor, &id)?;
    let key_of = |conn: &mut SqliteConnection, c: Option<CardId>| -> DbResult<Option<String>> {
        c.map(|c| {
            cards::table
                .find(c.to_string())
                .select(cards::sort_key)
                .first::<String>(conn)
                .map_err(DbError::from)
        })
        .transpose()
    };
    let lo = key_of(conn, m.before)?;
    let hi = key_of(conn, m.after)?;
    let key = order::key_between(lo.as_deref(), hi.as_deref());
    diesel::update(cards::table.find(&id))
        .set((
            cards::sort_key.eq(key),
            cards::updated_at.eq(now_millis()),
            cards::updated_by.eq(actor.user.to_string()),
            cards::rev.eq(cards::rev + 1),
        ))
        .execute(conn)?;
    let title: String = cards::table.find(&id).select(cards::title).first(conn)?;
    crate::activity::log(conn, actor, Some(&id), "move", &title)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::db_with_user;
    use cardfile_core::{CardFilter, NewCard};

    fn order_of(db: &Database, user: cardfile_core::UserId) -> Vec<String> {
        db.list_cards(user, &CardFilter::default())
            .unwrap()
            .into_iter()
            .map(|c| c.title)
            .collect()
    }

    #[test]
    fn applies_every_move_in_one_go() {
        let (db, user) = db_with_user();
        let actor = Actor::user(user.id);
        let mk = |t: &str| {
            db.create_card(
                &actor,
                NewCard {
                    title: t.into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .id
        };
        let (a, b, c) = (mk("a"), mk("b"), mk("c"));
        assert_eq!(order_of(&db, user.id), ["c", "b", "a"]);
        db.move_cards(
            &actor,
            &[
                CardMove {
                    id: b,
                    before: Some(a),
                    after: None,
                },
                CardMove {
                    id: c,
                    before: Some(b),
                    after: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(order_of(&db, user.id), ["a", "b", "c"]);
    }

    #[test]
    fn a_failing_move_rolls_back_the_earlier_ones() {
        let (db, user) = db_with_user();
        let actor = Actor::user(user.id);
        let mk = |t: &str| {
            db.create_card(
                &actor,
                NewCard {
                    title: t.into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .id
        };
        let (a, b, c) = (mk("a"), mk("b"), mk("c"));
        let before = order_of(&db, user.id);
        let rev = db.card(b).unwrap().rev;
        let missing = CardId::new();
        let err = db.move_cards(
            &actor,
            &[
                CardMove {
                    id: b,
                    before: Some(a),
                    after: None,
                },
                // Refers to a card that does not exist: fails after the first move.
                CardMove {
                    id: c,
                    before: Some(missing),
                    after: None,
                },
            ],
        );
        assert!(err.is_err());
        assert_eq!(order_of(&db, user.id), before);
        assert_eq!(db.card(b).unwrap().rev, rev);
    }
}
