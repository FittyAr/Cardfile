//! Card listing and full-text search (SQLite FTS5).

use cardfile_core::{CardFilter, CardSummary, LockMode, SearchScope, SortOrder, UserId};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Nullable, Text};

use crate::rows::parse_id;
use crate::{Database, DbResult};

/// Turn free text typed by the user into a safe FTS5 query.
///
/// Every word becomes a quoted prefix term (`"word"*`), so FTS5 operators typed by the
/// user are treated literally. Returns `None` when there is nothing to search for.
pub fn fts_query(input: &str, scope: SearchScope) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{w}\"*"))
        .collect();
    if terms.is_empty() {
        return None;
    }
    let joined = terms.join(" ");
    Some(match scope {
        SearchScope::TitleOnly => format!("title : ({joined})"),
        SearchScope::TitleAndContent => format!("{{title body}} : ({joined})"),
        SearchScope::Everything => joined,
    })
}

#[derive(QueryableByName, Debug)]
struct SummaryRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    plain_text: String,
    #[diesel(sql_type = Text)]
    lock_mode: String,
    #[diesel(sql_type = Bool)]
    is_pinned: bool,
    #[diesel(sql_type = Bool)]
    is_favorite: bool,
    #[diesel(sql_type = Text)]
    sort_key: String,
    #[diesel(sql_type = BigInt)]
    updated_at: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    deleted_at: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    snippet: Option<String>,
}

const PREVIEW_CHARS: usize = 140;

impl Database {
    /// List the owner's cards matching `filter`. Pinned cards come first.
    pub fn list_cards(&self, owner: UserId, filter: &CardFilter) -> DbResult<Vec<CardSummary>> {
        let fts = fts_query(&filter.query, filter.scope);
        self.with(|conn| {
            let mut sql = String::from(
                "SELECT c.id, c.title, c.plain_text, c.lock_mode, c.is_pinned, c.is_favorite, \
                 c.sort_key, c.updated_at, c.deleted_at, ",
            );
            if fts.is_some() {
                sql.push_str(
                    "snippet(cards_fts, -1, char(1), char(2), '…', 12) AS snippet \
                     FROM cards c JOIN cards_fts ON cards_fts.card_id = c.id ",
                );
            } else {
                sql.push_str("NULL AS snippet FROM cards c ");
            }
            sql.push_str("WHERE c.owner_id = ? ");
            sql.push_str(if filter.deleted {
                "AND c.deleted_at IS NOT NULL "
            } else {
                "AND c.deleted_at IS NULL "
            });
            if fts.is_some() {
                sql.push_str("AND cards_fts MATCH ? ");
            }
            if filter.favorites_only {
                sql.push_str("AND c.is_favorite = 1 ");
            }
            if filter.tag.is_some() {
                sql.push_str(
                    "AND EXISTS (SELECT 1 FROM card_tags t WHERE t.card_id = c.id AND t.tag_id = ?) ",
                );
            }
            sql.push_str("ORDER BY ");
            if filter.deleted {
                sql.push_str("c.deleted_at DESC");
            } else {
                sql.push_str("c.is_pinned DESC, ");
                if fts.is_some() && filter.sort == SortOrder::Manual {
                    sql.push_str("cards_fts.rank, ");
                }
                sql.push_str(match filter.sort {
                    SortOrder::Manual => "c.sort_key ASC",
                    SortOrder::UpdatedDesc => "c.updated_at DESC",
                    SortOrder::CreatedDesc => "c.created_at DESC",
                    SortOrder::TitleAsc => "c.title COLLATE NOCASE ASC",
                });
            }

            let mut query = diesel::sql_query(sql).into_boxed().bind::<Text, _>(owner.to_string());
            if let Some(fts) = &fts {
                query = query.bind::<Text, _>(fts.clone());
            }
            if let Some(tag) = filter.tag {
                query = query.bind::<Text, _>(tag.to_string());
            }
            let rows: Vec<SummaryRow> = query.load(conn)?;

            let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
            let mut tags = crate::cards::tags_of(conn, &ids)?;
            rows.into_iter()
                .map(|r| {
                    let lock_mode = LockMode::parse(&r.lock_mode).unwrap_or_default();
                    let hidden = lock_mode.hides_content();
                    Ok(CardSummary {
                        tags: tags.remove(&r.id).unwrap_or_default(),
                        id: parse_id(&r.id)?,
                        preview: if hidden {
                            String::new()
                        } else {
                            cardfile_doc::preview(&r.plain_text, PREVIEW_CHARS)
                        },
                        title: r.title,
                        lock_mode,
                        is_pinned: r.is_pinned,
                        is_favorite: r.is_favorite,
                        sort_key: r.sort_key,
                        updated_at: r.updated_at,
                        deleted_at: r.deleted_at,
                        snippet: r.snippet.filter(|s| s.contains('\u{1}')),
                    })
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use cardfile_core::NewCard;

    use super::*;
    use crate::test_util::*;

    #[test]
    fn builds_safe_queries() {
        assert_eq!(fts_query("  ", SearchScope::Everything), None);
        assert_eq!(
            fts_query("foo \"bar", SearchScope::Everything).unwrap(),
            "\"foo\"* \"bar\"*"
        );
        assert_eq!(
            fts_query("a OR b", SearchScope::TitleOnly).unwrap(),
            "title : (\"a\"* \"OR\"* \"b\"*)"
        );
        assert_eq!(fts_query("*** --", SearchScope::Everything), None);
    }

    fn card(db: &Database, user: UserId, title: &str, body: &str, lock: LockMode) {
        db.create_card(
            user,
            NewCard {
                title: title.into(),
                plain_text: body.into(),
                lock_mode: lock,
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn full_text_search_respects_scope_and_privacy() {
        let (db, user) = db_with_user();
        card(&db, user.id, "Groceries", "milk eggs café", LockMode::None);
        card(&db, user.id, "Ideas", "build a café", LockMode::None);
        card(
            &db,
            user.id,
            "Diary",
            "secret café visit",
            LockMode::Private,
        );
        let search = |q: &str, scope| -> Vec<String> {
            let f = CardFilter {
                query: q.into(),
                scope,
                ..Default::default()
            };
            let mut t: Vec<String> = db
                .list_cards(user.id, &f)
                .unwrap()
                .into_iter()
                .map(|c| c.title)
                .collect();
            t.sort();
            t
        };
        // Diacritics are folded; private content is not indexed.
        assert_eq!(
            search("cafe", SearchScope::TitleAndContent),
            ["Groceries", "Ideas"]
        );
        assert_eq!(search("gro", SearchScope::TitleOnly), ["Groceries"]);
        assert!(search("milk", SearchScope::TitleOnly).is_empty());
        assert_eq!(search("diary", SearchScope::TitleAndContent), ["Diary"]);
        let f = CardFilter {
            query: "milk".into(),
            ..Default::default()
        };
        let hit = &db.list_cards(user.id, &f).unwrap()[0];
        assert!(hit.snippet.as_deref().unwrap().contains("\u{1}milk\u{2}"));
        assert_eq!(hit.preview, "milk eggs café");
    }

    #[test]
    fn private_cards_have_no_preview() {
        let (db, user) = db_with_user();
        card(&db, user.id, "Diary", "secret", LockMode::Private);
        let list = db.list_cards(user.id, &CardFilter::default()).unwrap();
        assert_eq!(list[0].preview, "");
    }

    #[test]
    fn search_survives_vacuum() {
        let (db, user) = db_with_user();
        let titled = |t: &str| NewCard {
            title: t.into(),
            ..Default::default()
        };
        let a = db.create_card(user.id, titled("alpha")).unwrap();
        let b = db.create_card(user.id, titled("bravo")).unwrap();
        let c = db.create_card(user.id, titled("charlie")).unwrap();
        // Leave a hole in the rowids, then let VACUUM renumber them.
        db.delete_card(a.id).unwrap();
        db.purge_card(a.id).unwrap();
        db.with(|conn| {
            diesel::connection::SimpleConnection::batch_execute(conn, "VACUUM;")?;
            Ok(())
        })
        .unwrap();
        for (q, id) in [("bravo", b.id), ("charlie", c.id)] {
            let filter = CardFilter {
                query: q.into(),
                ..Default::default()
            };
            let hits = db.list_cards(user.id, &filter).unwrap();
            assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), [id], "{q}");
        }
    }
}
