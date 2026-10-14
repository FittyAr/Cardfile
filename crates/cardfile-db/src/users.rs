use cardfile_core::{CardId, EncryptedPayload, RevisionId, User, UserId, UserKind, now_millis};
use diesel::prelude::*;

use crate::rows::{UserRow, payload_json};
use crate::schema::{card_keys, card_revisions, cards, users};
use crate::{Database, DbError, DbResult};

/// Username of the built-in AI account. It can never sign in.
const AI_USERNAME: &str = "cardfile-ai";

/// Ciphertext rewritten by a key change: card content and revisions re-encrypted
/// (legacy password-derived cards move to per-card keys) and the card keys wrapped
/// for the owner. See [`Database::update_credentials`] and [`Database::apply_rekey`].
#[derive(Clone, Debug, Default)]
pub struct Rekey {
    pub cards: Vec<(CardId, EncryptedPayload)>,
    pub revisions: Vec<(RevisionId, EncryptedPayload)>,
    pub card_keys: Vec<(CardId, UserId, String)>,
}

/// Changes to a user's secrets, written atomically by [`Database::update_credentials`].
/// `None` leaves a field unchanged; `Some(None)` clears it.
#[derive(Clone, Debug, Default)]
pub struct CredentialUpdate {
    pub password_hash: Option<Option<String>>,
    pub lock_hash: Option<Option<String>>,
    /// Public key and sealed secret key.
    pub keys: Option<(String, EncryptedPayload)>,
    pub rekey: Rekey,
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
        .filter(users::kind.eq(UserKind::Human.as_str()))
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
            kind: UserKind::Human.as_str().to_owned(),
            lock_hash: None,
            public_key: None,
            secret_key: None,
        };
        self.tx(|conn| {
            if find_live_by_username(conn, &row.username)?.is_some()
                || username_key(&row.username) == AI_USERNAME
            {
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

    /// Live human accounts, oldest first (the AI account is never listed).
    pub fn list_users(&self) -> DbResult<Vec<User>> {
        self.with(|conn| {
            users::table
                .filter(users::deleted_at.is_null())
                .filter(users::kind.eq(UserKind::Human.as_str()))
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

    /// The built-in AI account, created on first use.
    pub fn ai_user(&self) -> DbResult<User> {
        self.tx(|conn| {
            let existing: Option<UserRow> = users::table
                .filter(users::kind.eq(UserKind::Ai.as_str()))
                .filter(users::deleted_at.is_null())
                .select(UserRow::as_select())
                .first(conn)
                .optional()?;
            if let Some(row) = existing {
                return row.try_into();
            }
            let now = now_millis();
            let row = UserRow {
                id: UserId::new().to_string(),
                username: AI_USERNAME.into(),
                display_name: "Cardfile AI".into(),
                password_hash: None,
                is_admin: false,
                settings: "{}".into(),
                created_at: now,
                updated_at: now,
                deleted_at: None,
                rev: 1,
                kind: UserKind::Ai.as_str().into(),
                lock_hash: None,
                public_key: None,
                secret_key: None,
            };
            diesel::insert_into(users::table)
                .values(&row)
                .returning(UserRow::as_returning())
                .get_result::<UserRow>(conn)?
                .try_into()
        })
    }

    /// Write new password hashes, key pair and re-encrypted ciphertext in one
    /// transaction: a partial update would leave cards no password can open.
    pub fn update_credentials(&self, user: UserId, update: &CredentialUpdate) -> DbResult<()> {
        let now = now_millis();
        self.tx(|conn| {
            apply_rekey(conn, &update.rekey, now)?;
            let user_id = user.to_string();
            let target = || users::table.find(&user_id);
            if let Some(hash) = &update.password_hash {
                diesel::update(target())
                    .set(users::password_hash.eq(hash.as_deref()))
                    .execute(conn)?;
            }
            if let Some(hash) = &update.lock_hash {
                diesel::update(target())
                    .set(users::lock_hash.eq(hash.as_deref()))
                    .execute(conn)?;
            }
            if let Some((public, secret)) = &update.keys {
                diesel::update(target())
                    .set((
                        users::public_key.eq(public),
                        users::secret_key.eq(payload_json(Some(secret))),
                    ))
                    .execute(conn)?;
            }
            diesel::update(target())
                .set((users::updated_at.eq(now), users::rev.eq(users::rev + 1)))
                .execute(conn)?;
            Ok(())
        })
    }

    /// Write re-encrypted ciphertext and card keys (e.g. a legacy card moved to a
    /// per-card key when it is unlocked) in one transaction.
    pub fn apply_rekey(&self, rekey: &Rekey) -> DbResult<()> {
        self.tx(|conn| apply_rekey(conn, rekey, now_millis()))
    }

    pub fn set_lock_hash(&self, id: UserId, hash: Option<&str>) -> DbResult<()> {
        self.with(|conn| {
            diesel::update(users::table.find(id.to_string()))
                .set((
                    users::lock_hash.eq(hash),
                    users::updated_at.eq(now_millis()),
                    users::rev.eq(users::rev + 1),
                ))
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

fn apply_rekey(conn: &mut SqliteConnection, rekey: &Rekey, now: i64) -> DbResult<()> {
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
    for (card, user, wrapped) in &rekey.card_keys {
        diesel::replace_into(card_keys::table)
            .values((
                card_keys::card_id.eq(card.to_string()),
                card_keys::user_id.eq(user.to_string()),
                card_keys::wrapped.eq(wrapped),
                card_keys::created_at.eq(now),
            ))
            .execute(conn)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_util::*;
    use cardfile_core::Actor;

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
    fn credentials_and_rekey_are_written_together() {
        use cardfile_core::{EncryptedPayload, NewCard};

        let (db, alice) = db_with_user();
        let card = db
            .create_card(&Actor::user(alice.id), NewCard::default())
            .unwrap();
        let payload = EncryptedPayload {
            v: 2,
            salt: String::new(),
            nonce: "n".into(),
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
            ciphertext: "c".into(),
        };
        let update = super::CredentialUpdate {
            password_hash: Some(Some("h1".into())),
            lock_hash: Some(Some("l1".into())),
            keys: Some(("pub".into(), payload.clone())),
            rekey: super::Rekey {
                cards: vec![(card.id, payload.clone())],
                revisions: vec![],
                card_keys: vec![(card.id, alice.id, "wrapped".into())],
            },
        };
        db.update_credentials(alice.id, &update).unwrap();
        assert_eq!(db.card(card.id).unwrap().encrypted, Some(payload.clone()));
        let user = db.user(alice.id).unwrap();
        assert_eq!(user.password_hash.as_deref(), Some("h1"));
        assert_eq!(user.lock_hash.as_deref(), Some("l1"));
        assert_eq!(user.effective_lock_hash(), Some("l1"));
        assert_eq!(user.public_key.as_deref(), Some("pub"));
        assert_eq!(user.secret_key, Some(payload));
        assert_eq!(
            db.wrapped_card_key(card.id, alice.id).unwrap().as_deref(),
            Some("wrapped")
        );
        // Clearing the separate lock password falls back to the sign-in password.
        db.set_lock_hash(alice.id, None).unwrap();
        assert_eq!(db.user(alice.id).unwrap().effective_lock_hash(), Some("h1"));
    }

    #[test]
    fn ai_account_is_hidden() {
        let (db, _alice) = db_with_user();
        let ai = db.ai_user().unwrap();
        assert_eq!(ai.kind, cardfile_core::UserKind::Ai);
        assert_eq!(db.ai_user().unwrap().id, ai.id);
        assert_eq!(db.list_users().unwrap().len(), 1);
        assert!(db.user_by_username("cardfile-ai").unwrap().is_none());
        assert!(db.create_user("cardfile-ai", "x", None, false).is_err());
    }
}
