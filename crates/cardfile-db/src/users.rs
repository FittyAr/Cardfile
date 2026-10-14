use cardfile_core::{CardId, EncryptedPayload, RevisionId, User, UserId, UserSettings, now_millis};
use diesel::prelude::*;

use crate::rows::{UserRow, payload_json};
use crate::schema::{card_revisions, cards, users};
use crate::{Database, DbError, DbResult};

/// Where a user's password hash is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordSlot {
    /// The account password (password sign-in modes).
    Account,
    /// The lock password kept in user settings ("no password" sign-in).
    LockSetting,
}

/// Ciphertext re-encrypted under a new password, see [`Database::change_password`].
#[derive(Clone, Debug, Default)]
pub struct Rekey {
    pub cards: Vec<(CardId, EncryptedPayload)>,
    pub revisions: Vec<(RevisionId, EncryptedPayload)>,
}

/// Usernames compare case-insensitively across all of Unicode. SQLite's `lower()` and
/// `COLLATE NOCASE` only fold ASCII ("JOSÉ" ≠ "josé"), so matching happens in Rust; a
/// handful of accounts per file makes the scan cheap.
fn username_key(username: &str) -> String {
    username.trim().to_lowercase()
}

fn find_live_by_username(conn: &mut SqliteConnection, username: &str) -> DbResult<Option<UserRow>> {
    let key = username_key(username);
    Ok(users::table
        .filter(users::deleted_at.is_null())
        .select(UserRow::as_select())
        .load(conn)?
        .into_iter()
        .find(|row| username_key(&row.username) == key))
}

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
        self.tx(|conn| {
            if find_live_by_username(conn, &row.username)?.is_some() {
                return Err(DbError::Query(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    Box::new(format!("username already taken: {}", row.username)),
                )));
            }
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
        self.with(|conn| {
            find_live_by_username(conn, username)?
                .map(TryInto::try_into)
                .transpose()
        })
    }

    /// Store a new password hash together with every ciphertext re-encrypted under it,
    /// in one transaction: a partial rekey would leave cards no password can open.
    pub fn change_password(
        &self,
        user: UserId,
        slot: PasswordSlot,
        hash: &str,
        rekey: &Rekey,
    ) -> DbResult<()> {
        let now = now_millis();
        self.tx(|conn| {
            for (id, payload) in &rekey.cards {
                diesel::update(cards::table.find(id.to_string()))
                    .set((
                        cards::encrypted.eq(payload_json(Some(payload))),
                        cards::updated_at.eq(now),
                        cards::rev.eq(cards::rev + 1),
                    ))
                    .execute(conn)?;
            }
            for (id, payload) in &rekey.revisions {
                diesel::update(card_revisions::table.find(id.to_string()))
                    .set(card_revisions::encrypted.eq(payload_json(Some(payload))))
                    .execute(conn)?;
            }
            let user_id = user.to_string();
            let target = || users::table.find(&user_id);
            match slot {
                PasswordSlot::Account => {
                    diesel::update(target())
                        .set(users::password_hash.eq(hash))
                        .execute(conn)?;
                }
                PasswordSlot::LockSetting => {
                    let json: String = target().select(users::settings).first(conn)?;
                    let mut settings: UserSettings =
                        serde_json::from_str(&json).unwrap_or_default();
                    settings.lock_password_hash = Some(hash.to_owned());
                    let json = serde_json::to_string(&settings)
                        .map_err(|e| DbError::Corrupt(e.to_string()))?;
                    diesel::update(target())
                        .set(users::settings.eq(json))
                        .execute(conn)?;
                }
            }
            diesel::update(target())
                .set((users::updated_at.eq(now), users::rev.eq(users::rev + 1)))
                .execute(conn)?;
            Ok(())
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

    #[test]
    fn usernames_fold_non_ascii_case() {
        let db = db();
        let jose = db.create_user("JOSÉ", "José", None, false).unwrap();
        assert_eq!(
            db.user_by_username("josé").unwrap().map(|u| u.id),
            Some(jose.id)
        );
        let err = db.create_user("josé", "Other", None, false).unwrap_err();
        assert!(err.is_unique_violation());
    }

    #[test]
    fn change_password_rewrites_payloads_and_hash_together() {
        use cardfile_core::{EncryptedPayload, NewCard};

        let (db, alice) = db_with_user();
        let card = db.create_card(alice.id, NewCard::default()).unwrap();
        let payload = EncryptedPayload {
            v: 1,
            salt: "s".into(),
            nonce: "n".into(),
            m_cost: 1,
            t_cost: 1,
            p_cost: 1,
            ciphertext: "c".into(),
        };
        let rekey = super::Rekey {
            cards: vec![(card.id, payload.clone())],
            revisions: vec![],
        };
        db.change_password(alice.id, super::PasswordSlot::LockSetting, "h1", &rekey)
            .unwrap();
        assert_eq!(db.card(card.id).unwrap().encrypted, Some(payload));
        assert_eq!(
            db.user_settings(alice.id)
                .unwrap()
                .lock_password_hash
                .as_deref(),
            Some("h1")
        );
        db.change_password(
            alice.id,
            super::PasswordSlot::Account,
            "h2",
            &super::Rekey::default(),
        )
        .unwrap();
        assert_eq!(
            db.user(alice.id).unwrap().password_hash.as_deref(),
            Some("h2")
        );
    }
}
