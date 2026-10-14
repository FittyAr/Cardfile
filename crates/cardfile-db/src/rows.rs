//! Row structs mapped by Diesel and conversions to domain types.

use cardfile_core::{
    Card, CardId, CardRevision, EncryptedPayload, LockMode, RevisionId, Tag, TagId, User, UserId,
};
use diesel::prelude::*;

use crate::schema::{card_revisions, cards, tags, users};
use crate::{DbError, DbResult};

pub(crate) fn parse_id<T: std::str::FromStr>(s: &str) -> DbResult<T> {
    s.parse()
        .map_err(|_| DbError::Corrupt(format!("invalid id `{s}`")))
}

pub(crate) fn parse_payload(json: Option<&str>) -> DbResult<Option<EncryptedPayload>> {
    json.map(|j| serde_json::from_str(j).map_err(|e| DbError::Corrupt(e.to_string())))
        .transpose()
}

pub(crate) fn payload_json(p: Option<&EncryptedPayload>) -> Option<String> {
    p.and_then(|p| serde_json::to_string(p).ok())
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct UserRow {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub settings: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub rev: i64,
}

impl TryFrom<UserRow> for User {
    type Error = DbError;
    fn try_from(r: UserRow) -> DbResult<Self> {
        Ok(User {
            id: parse_id::<UserId>(&r.id)?,
            username: r.username,
            display_name: r.display_name,
            password_hash: r.password_hash,
            is_admin: r.is_admin,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = cards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct CardRow {
    pub id: String,
    pub owner_id: String,
    pub title: String,
    pub content: String,
    pub plain_text: String,
    pub lock_mode: String,
    pub encrypted: Option<String>,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub sort_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub rev: i64,
}

impl CardRow {
    pub fn into_card(self, tags: Vec<TagId>) -> DbResult<Card> {
        Ok(Card {
            id: parse_id::<CardId>(&self.id)?,
            owner_id: parse_id::<UserId>(&self.owner_id)?,
            title: self.title,
            content: self.content,
            plain_text: self.plain_text,
            lock_mode: LockMode::parse(&self.lock_mode).unwrap_or_default(),
            encrypted: parse_payload(self.encrypted.as_deref())?,
            is_pinned: self.is_pinned,
            is_favorite: self.is_favorite,
            sort_key: self.sort_key,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            rev: self.rev,
            tags,
        })
    }
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = card_revisions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct RevisionRow {
    pub id: String,
    pub card_id: String,
    pub title: String,
    pub content: String,
    pub encrypted: Option<String>,
    pub created_at: i64,
}

impl TryFrom<RevisionRow> for CardRevision {
    type Error = DbError;
    fn try_from(r: RevisionRow) -> DbResult<Self> {
        Ok(CardRevision {
            id: parse_id::<RevisionId>(&r.id)?,
            card_id: parse_id::<CardId>(&r.card_id)?,
            title: r.title,
            content: r.content,
            encrypted: parse_payload(r.encrypted.as_deref())?,
            created_at: r.created_at,
        })
    }
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct TagRow {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub color: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub rev: i64,
}

impl TryFrom<TagRow> for Tag {
    type Error = DbError;
    fn try_from(r: TagRow) -> DbResult<Self> {
        Ok(Tag {
            id: parse_id::<TagId>(&r.id)?,
            owner_id: parse_id::<UserId>(&r.owner_id)?,
            name: r.name,
            color: r.color,
        })
    }
}
