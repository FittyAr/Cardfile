use cardfile_core::{AppSettings, UserId, UserSettings, now_millis};
use diesel::prelude::*;
use diesel::upsert::excluded;

use crate::schema::{app_settings, users};
use crate::{Database, DbError, DbResult};

const APP_SETTINGS_KEY: &str = "app";

impl Database {
    /// Installation-wide settings; defaults when never saved.
    pub fn app_settings(&self) -> DbResult<AppSettings> {
        self.with(|conn| {
            let value: Option<String> = app_settings::table
                .find(APP_SETTINGS_KEY)
                .select(app_settings::value)
                .first(conn)
                .optional()?;
            Ok(value
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default())
        })
    }

    pub fn save_app_settings(&self, settings: &AppSettings) -> DbResult<()> {
        let value = serde_json::to_string(settings).map_err(|e| DbError::Corrupt(e.to_string()))?;
        self.with(|conn| {
            diesel::insert_into(app_settings::table)
                .values((
                    app_settings::key.eq(APP_SETTINGS_KEY),
                    app_settings::value.eq(&value),
                    app_settings::updated_at.eq(now_millis()),
                ))
                .on_conflict(app_settings::key)
                .do_update()
                .set((
                    app_settings::value.eq(excluded(app_settings::value)),
                    app_settings::updated_at.eq(excluded(app_settings::updated_at)),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn user_settings(&self, user: UserId) -> DbResult<UserSettings> {
        self.with(|conn| {
            let json: String = users::table
                .find(user.to_string())
                .select(users::settings)
                .first(conn)?;
            Ok(serde_json::from_str(&json).unwrap_or_default())
        })
    }

    pub fn save_user_settings(&self, user: UserId, settings: &UserSettings) -> DbResult<()> {
        let json = serde_json::to_string(settings).map_err(|e| DbError::Corrupt(e.to_string()))?;
        self.with(|conn| {
            diesel::update(users::table.find(user.to_string()))
                .set((
                    users::settings.eq(json),
                    users::updated_at.eq(now_millis()),
                    users::rev.eq(users::rev + 1),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use cardfile_core::AuthMode;

    use crate::test_util::*;

    #[test]
    fn app_settings_roundtrip() {
        let db = db();
        assert!(!db.app_settings().unwrap().initialized);
        let mut s = db.app_settings().unwrap();
        s.initialized = true;
        s.auth_mode = AuthMode::MultiUser;
        db.save_app_settings(&s).unwrap();
        db.save_app_settings(&s).unwrap();
        assert_eq!(db.app_settings().unwrap(), s);
    }

    #[test]
    fn user_settings_roundtrip() {
        let (db, user) = db_with_user();
        let mut s = db.user_settings(user.id).unwrap();
        s.theme = "Forest".into();
        db.save_user_settings(user.id, &s).unwrap();
        assert_eq!(db.user_settings(user.id).unwrap().theme, "Forest");
    }
}
