use cardfile_core::{User, UserId, now_millis};
use diesel::prelude::*;

use crate::rows::UserRow;
use crate::schema::users;
use crate::search::lower;
use crate::{Database, DbResult};

impl Database {
    /// Create an account. `password_hash` is an Argon2 PHC string or `None`.
    pub fn create_user(
        &self,
        username: &str,
        display_name: &str,
        password_hash: Option<&str>,
        is_admin: bool,
    ) -> DbResult<User> {
        let now = now_millis();
        let row = UserRow {
            id: UserId::new().to_string(),
            username: username.trim().to_owned(),
            display_name: display_name.trim().to_owned(),
            password_hash: password_hash.map(str::to_owned),
            is_admin,
            settings: "{}".into(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            rev: 1,
        };
        self.with(|conn| {
            let row: UserRow = diesel::insert_into(users::table)
                .values(&row)
                .returning(UserRow::as_returning())
                .get_result(conn)?;
            row.try_into()
        })
    }

    pub fn list_users(&self) -> DbResult<Vec<User>> {
        self.with(|conn| {
            users::table
                .filter(users::deleted_at.is_null())
                .order(users::created_at.asc())
                .select(UserRow::as_select())
                .load(conn)?
                .into_iter()
                .map(TryInto::try_into)
                .collect()
        })
    }

    pub fn user(&self, id: UserId) -> DbResult<User> {
        self.with(|conn| {
            users::table
                .find(id.to_string())
                .select(UserRow::as_select())
                .first(conn)?
                .try_into()
        })
    }

    /// Case-insensitive lookup of a live account.
    pub fn user_by_username(&self, username: &str) -> DbResult<Option<User>> {
        let username = username.trim().to_lowercase();
        self.with(|conn| {
            users::table
                .filter(users::deleted_at.is_null())
                .filter(lower(users::username).eq(username))
                .select(UserRow::as_select())
                .first(conn)
                .optional()?
                .map(TryInto::try_into)
                .transpose()
        })
    }

    pub fn set_password_hash(&self, id: UserId, hash: Option<&str>) -> DbResult<()> {
        self.with(|conn| {
            diesel::update(users::table.find(id.to_string()))
                .set((
                    users::password_hash.eq(hash),
                    users::updated_at.eq(now_millis()),
                    users::rev.eq(users::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn update_user(&self, id: UserId, display_name: &str, is_admin: bool) -> DbResult<()> {
        self.with(|conn| {
            diesel::update(users::table.find(id.to_string()))
                .set((
                    users::display_name.eq(display_name.trim()),
                    users::is_admin.eq(is_admin),
                    users::updated_at.eq(now_millis()),
                    users::rev.eq(users::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    /// Soft-delete an account (its cards stay in the database).
    pub fn delete_user(&self, id: UserId) -> DbResult<()> {
        let now = now_millis();
        self.with(|conn| {
            diesel::update(users::table.find(id.to_string()))
                .set((
                    users::deleted_at.eq(now),
                    users::updated_at.eq(now),
                    users::rev.eq(users::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::*;

    #[test]
    fn create_and_find_users() {
        let (db, alice) = db_with_user();
        assert_eq!(db.user(alice.id).unwrap().username, "alice");
        assert_eq!(
            db.user_by_username("ALICE").unwrap().map(|u| u.id),
            Some(alice.id)
        );
        assert!(db.user_by_username("nobody").unwrap().is_none());
        let err = db.create_user("Alice", "Other", None, false).unwrap_err();
        assert!(err.is_unique_violation());
        db.set_password_hash(alice.id, Some("hash")).unwrap();
        assert_eq!(
            db.user(alice.id).unwrap().password_hash.as_deref(),
            Some("hash")
        );
        db.delete_user(alice.id).unwrap();
        assert!(db.list_users().unwrap().is_empty());
        // Username is free again after deletion.
        db.create_user("alice", "New", None, false).unwrap();
    }
}
