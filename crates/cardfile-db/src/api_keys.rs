//! API keys that let AI clients act on behalf of a user.

use cardfile_core::{ApiKey, ApiKeyId, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::parse_id;
use crate::schema::{api_keys, users};
use crate::{Database, DbResult};

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = api_keys)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct ApiKeyRow {
    id: String,
    user_id: String,
    name: String,
    prefix: String,
    key_hash: String,
    allow_private: bool,
    read_only: bool,
    created_at: i64,
    last_used_at: Option<i64>,
    revoked_at: Option<i64>,
}

impl TryFrom<ApiKeyRow> for ApiKey {
    type Error = crate::DbError;
    fn try_from(r: ApiKeyRow) -> DbResult<Self> {
        Ok(ApiKey {
            id: parse_id::<ApiKeyId>(&r.id)?,
            user_id: parse_id::<UserId>(&r.user_id)?,
            name: r.name,
            prefix: r.prefix,
            allow_private: r.allow_private,
            read_only: r.read_only,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
            revoked_at: r.revoked_at,
        })
    }
}

impl Database {
    /// Store a new key. `key_hash` is the BLAKE3 hash of the secret (see
    /// `cardfile_crypto::new_api_key`); `prefix` its first characters.
    pub fn create_api_key(
        &self,
        user: UserId,
        name: &str,
        prefix: &str,
        key_hash: &str,
        allow_private: bool,
        read_only: bool,
    ) -> DbResult<ApiKey> {
        let row = ApiKeyRow {
            id: ApiKeyId::new().to_string(),
            user_id: user.to_string(),
            name: name.trim().to_owned(),
            prefix: prefix.to_owned(),
            key_hash: key_hash.to_owned(),
            allow_private,
            read_only,
            created_at: now_millis(),
            last_used_at: None,
            revoked_at: None,
        };
        self.with(|conn| {
            diesel::insert_into(api_keys::table)
                .values(&row)
                .returning(ApiKeyRow::as_returning())
                .get_result(conn)?
                .try_into()
        })
    }

    /// The user's keys that were not revoked, newest first.
    pub fn list_api_keys(&self, user: UserId) -> DbResult<Vec<ApiKey>> {
        self.with(|conn| {
            api_keys::table
                .filter(api_keys::user_id.eq(user.to_string()))
                .filter(api_keys::revoked_at.is_null())
                .order(api_keys::created_at.desc())
                .select(ApiKeyRow::as_select())
                .load(conn)?
                .into_iter()
                .map(TryInto::try_into)
                .collect()
        })
    }

    /// Find a live key by the hash of its secret and record that it was used. Keys
    /// of deleted accounts never authenticate, even if they were not revoked.
    pub fn use_api_key(&self, key_hash: &str) -> DbResult<Option<ApiKey>> {
        self.with(|conn| {
            let live_users = users::table
                .filter(users::deleted_at.is_null())
                .select(users::id);
            let row: Option<ApiKeyRow> = api_keys::table
                .filter(api_keys::key_hash.eq(key_hash))
                .filter(api_keys::revoked_at.is_null())
                .filter(api_keys::user_id.eq_any(live_users))
                .select(ApiKeyRow::as_select())
                .first(conn)
                .optional()?;
            let Some(row) = row else { return Ok(None) };
            diesel::update(api_keys::table.find(&row.id))
                .set(api_keys::last_used_at.eq(now_millis()))
                .execute(conn)?;
            row.try_into().map(Some)
        })
    }

    /// Revoke a key; it stops working immediately. Only its owner may revoke it.
    pub fn revoke_api_key(&self, user: UserId, id: ApiKeyId) -> DbResult<()> {
        self.with(|conn| {
            diesel::update(
                api_keys::table
                    .find(id.to_string())
                    .filter(api_keys::user_id.eq(user.to_string())),
            )
            .set(api_keys::revoked_at.eq(now_millis()))
            .execute(conn)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::*;

    #[test]
    fn keys_lifecycle() {
        let (db, alice) = db_with_user();
        let key = db
            .create_api_key(alice.id, "Claude", "cf_abcd", "hash1", false, true)
            .unwrap();
        assert!(key.read_only && !key.allow_private);
        assert_eq!(db.list_api_keys(alice.id).unwrap().len(), 1);
        let used = db.use_api_key("hash1").unwrap().unwrap();
        assert_eq!(used.id, key.id);
        assert!(
            db.list_api_keys(alice.id).unwrap()[0]
                .last_used_at
                .is_some()
        );
        assert!(db.use_api_key("other").unwrap().is_none());
        db.revoke_api_key(alice.id, key.id).unwrap();
        assert!(db.use_api_key("hash1").unwrap().is_none());
        assert!(db.list_api_keys(alice.id).unwrap().is_empty());
    }

    #[test]
    fn deleted_users_keys_stop_working() {
        let (db, alice) = db_with_user();
        db.create_api_key(alice.id, "Claude", "cf_abcd", "hash1", false, false)
            .unwrap();
        assert!(db.use_api_key("hash1").unwrap().is_some());
        db.delete_user(alice.id).unwrap();
        assert!(db.use_api_key("hash1").unwrap().is_none());
        assert!(db.list_api_keys(alice.id).unwrap().is_empty());
    }

    #[test]
    fn keys_of_a_deleted_owner_are_refused_even_unrevoked() {
        use diesel::prelude::*;
        let (db, alice) = db_with_user();
        db.create_api_key(alice.id, "Claude", "cf_abcd", "hash1", false, false)
            .unwrap();
        // Simulate an account deleted by an older version that did not revoke keys.
        db.with(|conn| {
            diesel::sql_query("UPDATE users SET deleted_at = 1").execute(conn)?;
            Ok(())
        })
        .unwrap();
        assert!(db.use_api_key("hash1").unwrap().is_none());
    }
}
