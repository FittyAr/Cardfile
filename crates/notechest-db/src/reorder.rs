//! Moving several cards in the manual order as one atomic change.
//!
//! [`Database::move_card`] commits each move on its own. Callers that rearrange
//! many cards at once (the AI's `reorder_cards` tool) use [`Database::move_cards`]
//! so that a failure half-way leaves the order exactly as it was, instead of a
//! partially applied arrangement.

use notechest_core::{Actor, CardId};

use crate::{Database, DbResult};

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
                crate::cards::move_card_in(conn, actor, m.id, m.before, m.after)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::db_with_user;
    use notechest_core::{CardFilter, NewCard};

    fn order_of(db: &Database, user: notechest_core::UserId) -> Vec<String> {
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
