use cardfile_core::{AppSettings, UserId, UserSettings, now_millis};
use diesel::prelude::*;
use diesel::upsert::excluded;

use crate::schema::{app_settings, users};
use crate::{Database, DbError, DbResult};

const APP_SETTINGS_KEY: &str = "app";

impl Database {
    /// Installation-wide settings; defaults when never saved.
    ///
    /// Invalid fields fall back to their defaults one by one; a stored value that is
    /// not a JSON object at all is reported as [`DbError::Corrupt`] instead of being
    /// replaced by defaults, which would re-run the first-run wizard on an existing
    /// installation.
    pub fn app_settings(&self) -> DbResult<AppSettings> {
        self.with(|conn| {
            let value: Option<String> = app_settings::table
                .find(APP_SETTINGS_KEY)
                .select(app_settings::value)
                .first(conn)
                .optional()?;
            value.map_or_else(
                || Ok(AppSettings::default()),
                |v| {
                    AppSettings::from_json(&v)
                        .map_err(|e| DbError::Corrupt(format!("app settings: {e}")))
                },
            )
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

    /// A user's preferences. Invalid fields fall back to their defaults one by one; a
    /// value that is not a JSON object is [`DbError::Corrupt`] (returning defaults
    /// would erase e.g. the configured AI providers on the next save).
    pub fn user_settings(&self, user: UserId) -> DbResult<UserSettings> {
        self.with(|conn| {
            let json: String = users::table
                .find(user.to_string())
                .select(users::settings)
                .first(conn)?;
            UserSettings::from_json(&json)
                .map_err(|e| DbError::Corrupt(format!("user settings: {e}")))
        })
    }

    /// Store a user's preferences. Non-finite sizes are replaced by their defaults
    /// first: JSON cannot hold NaN.
    pub fn save_user_settings(&self, user: UserId, settings: &UserSettings) -> DbResult<()> {
        let json = serde_json::to_string(&settings.sanitized())
            .map_err(|e| DbError::Corrupt(e.to_string()))?;
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
    fn damaged_settings_are_errors_not_defaults() {
        use diesel::prelude::*;
        let (db, user) = db_with_user();
        let mut s = db.app_settings().unwrap();
        s.initialized = true;
        db.save_app_settings(&s).unwrap();
        // A field from a future version does not reset the others.
        db.with(|conn| {
            diesel::sql_query(
                "UPDATE app_settings SET value = json_set(value, '$.auth_mode', 'future', \
                 '$.enabled_lock_modes', json('[\"private\",\"future\"]'))",
            )
            .execute(conn)?;
            Ok(())
        })
        .unwrap();
        let read = db.app_settings().unwrap();
        assert!(read.initialized);
        assert_eq!(read.enabled_lock_modes, [cardfile_core::LockMode::Private]);
        // Garbage is reported, never silently replaced by "not initialized".
        db.with(|conn| {
            diesel::sql_query("UPDATE app_settings SET value = 'garbage'").execute(conn)?;
            diesel::sql_query("UPDATE users SET settings = '[]'").execute(conn)?;
            Ok(())
        })
        .unwrap();
        assert!(db.app_settings().is_err());
        assert!(db.user_settings(user.id).is_err());
    }

    #[test]
    fn nan_sizes_do_not_break_user_settings() {
        let (db, user) = db_with_user();
        let mut s = db.user_settings(user.id).unwrap();
        s.ui_font_size = f32::NAN;
        s.ai_providers = vec![serde_json::json!({"id": "p"})];
        db.save_user_settings(user.id, &s).unwrap();
        let read = db.user_settings(user.id).unwrap();
        assert_eq!(read.ui_font_size, 14.0);
        assert_eq!(read.ai_providers.len(), 1);
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
