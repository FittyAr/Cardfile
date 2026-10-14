//! Activity log: who (or which AI session) changed what.

use cardfile_core::{ActivityEntry, ActivityId, Actor, CardId, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::parse_id;
use crate::schema::activity;
use crate::{Database, DbResult};

/// Repeated actions of the same kind by the same actor on the same card within this
/// window are merged into one entry, so autosave does not flood the log.
const COALESCE_MS: i64 = 10 * 60 * 1000;

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = activity)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct ActivityRow {
    id: String,
    card_id: Option<String>,
    actor_id: String,
    on_behalf_of: Option<String>,
    via: Option<String>,
    action: String,
    summary: String,
    created_at: i64,
}

impl TryFrom<ActivityRow> for ActivityEntry {
    type Error = crate::DbError;
    fn try_from(r: ActivityRow) -> DbResult<Self> {
        Ok(ActivityEntry {
            id: parse_id::<ActivityId>(&r.id)?,
            card_id: r.card_id.as_deref().map(parse_id::<CardId>).transpose()?,
            actor_id: parse_id::<UserId>(&r.actor_id)?,
            on_behalf_of: r
                .on_behalf_of
                .as_deref()
                .map(parse_id::<UserId>)
                .transpose()?,
            via: r.via,
            action: r.action,
            summary: r.summary,
            created_at: r.created_at,
        })
    }
}

/// Record `action` by `actor`, merging it into the previous entry when it repeats.
pub(crate) fn log(
    conn: &mut SqliteConnection,
    actor: &Actor,
    card: Option<&str>,
    action: &str,
    summary: &str,
) -> DbResult<()> {
    let now = now_millis();
    let on_behalf = actor.on_behalf_of.map(|u| u.to_string());
    if let Some(card_id) = card {
        let last: Option<ActivityRow> = activity::table
            .filter(activity::card_id.eq(card_id))
            .order(activity::created_at.desc())
            .select(ActivityRow::as_select())
            .first(conn)
            .optional()?;
        if let Some(last) = last
            && last.actor_id == actor.user.to_string()
            && last.on_behalf_of == on_behalf
            && last.via == actor.via
            && last.action == action
            && now - last.created_at < COALESCE_MS
        {
            diesel::update(activity::table.find(&last.id))
                .set((activity::created_at.eq(now), activity::summary.eq(summary)))
                .execute(conn)?;
            return Ok(());
        }
    }
    diesel::insert_into(activity::table)
        .values(&ActivityRow {
            id: ActivityId::new().to_string(),
            card_id: card.map(str::to_owned),
            actor_id: actor.user.to_string(),
            on_behalf_of: on_behalf,
            via: actor.via.clone(),
            action: action.to_owned(),
            summary: summary.to_owned(),
            created_at: now,
        })
        .execute(conn)?;
    Ok(())
}

impl Database {
    /// Newest entries first. `card` limits to one card; `principal` to changes made on
    /// behalf of (or by) one person.
    pub fn activity(
        &self,
        card: Option<CardId>,
        principal: Option<UserId>,
        limit: i64,
    ) -> DbResult<Vec<ActivityEntry>> {
        self.with(|conn| {
            let mut q = activity::table
                .order(activity::created_at.desc())
                .limit(limit)
                .select(ActivityRow::as_select())
                .into_boxed();
            if let Some(card) = card {
                q = q.filter(activity::card_id.eq(card.to_string()));
            }
            if let Some(p) = principal {
                let p = p.to_string();
                q = q.filter(
                    activity::on_behalf_of
                        .eq(p.clone())
                        .or(activity::actor_id.eq(p)),
                );
            }
            q.load(conn)?.into_iter().map(TryInto::try_into).collect()
        })
    }

    /// Record an action that is not a card change (e.g. an AI search).
    pub fn log_activity(
        &self,
        actor: &Actor,
        card: Option<CardId>,
        action: &str,
        summary: &str,
    ) -> DbResult<()> {
        let card = card.map(|c| c.to_string());
        self.with(|conn| log(conn, actor, card.as_deref(), action, summary))
    }
}

#[cfg(test)]
mod tests {
    use cardfile_core::{Actor, NewCard};

    use crate::test_util::*;

    #[test]
    fn changes_are_attributed_and_coalesced() {
        let (db, alice) = db_with_user();
        let ai = db.ai_user().unwrap();
        let me = Actor::user(alice.id);
        let bot = Actor::ai(ai.id, alice.id, "Claude");
        let card = db.create_card(&me, NewCard::default()).unwrap();
        let upd = |t: &str| crate::ContentUpdate {
            title: t.into(),
            ..Default::default()
        };
        db.update_card_content(&bot, card.id, upd("a"), 0, 0)
            .unwrap();
        db.update_card_content(&bot, card.id, upd("b"), 0, 0)
            .unwrap();
        let card = db.card(card.id).unwrap();
        assert_eq!(card.updated_by, Some(ai.id));
        let log = db.activity(Some(card.id), None, 10).unwrap();
        // create + one merged edit.
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].action, "edit");
        assert_eq!(log[0].actor_id, ai.id);
        assert_eq!(log[0].on_behalf_of, Some(alice.id));
        assert_eq!(log[0].via.as_deref(), Some("Claude"));
        assert_eq!(log[0].summary, "b");
        assert_eq!(db.activity(None, Some(alice.id), 10).unwrap().len(), 2);
    }
}
