//! Device/storage preferences, deliberately independent of the restored database.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Global};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// Normal window dimensions in logical pixels, plus its last maximized state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct WindowState {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1180.,
            height: 760.,
            maximized: false,
        }
    }
}

impl WindowState {
    pub fn clamped(&self, available: (f32, f32)) -> (f32, f32) {
        let d = Self::default();
        let clamp = |n: f32, default: f32, minimum: f32, maximum: f32| {
            let maximum = maximum.max(1.);
            (if n.is_finite() && n > 0. { n } else { default }).clamp(minimum.min(maximum), maximum)
        };
        (
            clamp(self.width, d.width, 760., available.0),
            clamp(self.height, d.height, 480., available.1),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct LocalConfig {
    pub version: u32,
    pub collection: String,
    pub window: WindowState,
    pub backup_directory: PathBuf,
    pub daily_backup: bool,
    pub keep_automatic: usize,
    pub last_automatic: Option<i64>,
    pub last_result: Option<String>,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            version: 1,
            collection: uuid::Uuid::now_v7().to_string(),
            window: WindowState::default(),
            backup_directory: "backups".into(),
            daily_backup: false,
            keep_automatic: 10,
            last_automatic: None,
            last_result: None,
        }
    }
}

impl LocalConfig {
    pub fn load(root: &Path) -> Self {
        std::fs::read(root.join("local-config.json"))
            .ok()
            .and_then(|s| serde_json::from_slice::<Self>(&s).ok())
            .filter(|s| s.version == 1 && uuid::Uuid::parse_str(&s.collection).is_ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        use std::io::Write as _;
        std::fs::create_dir_all(root)?;
        let mut tmp = tempfile::NamedTempFile::new_in(root)?;
        serde_json::to_writer_pretty(&mut tmp, self)?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;
        tmp.persist(root.join("local-config.json"))
            .map_err(|e| e.error)?;
        Ok(())
    }

    pub fn backup_dir(&self, root: &Path) -> PathBuf {
        root.join(&self.backup_directory)
    }

    pub fn automatic_due(&self, now: i64) -> bool {
        self.daily_backup
            && self
                .last_automatic
                .is_none_or(|last| now.saturating_sub(last) >= 86_400_000)
    }
}

#[derive(Clone)]
pub(crate) struct LocalPreferences {
    pub root: PathBuf,
    pub config: Arc<Mutex<LocalConfig>>,
    pub writer: Arc<Mutex<()>>,
    pub generation: Arc<std::sync::atomic::AtomicU64>,
}
impl Global for LocalPreferences {}

impl LocalPreferences {
    pub fn persist(&self) -> std::io::Result<()> {
        let _writer = self.writer.lock();
        let config = self.config.lock().clone();
        config.save(&self.root)
    }
    pub fn snapshot(cx: &App) -> LocalConfig {
        cx.global::<Self>().config.lock().clone()
    }

    pub fn change(cx: &mut App, f: impl FnOnce(&mut LocalConfig)) {
        let prefs = cx.global::<Self>().clone();
        f(&mut prefs.config.lock());
        let ticket = prefs
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        cx.background_spawn(async move {
            let _writer = prefs.writer.lock();
            if prefs.generation.load(std::sync::atomic::Ordering::SeqCst) != ticket {
                return;
            }
            // The UI never waits on the disk writer's lock.
            let config = prefs.config.lock().clone();
            if let Err(e) = config.save(&prefs.root) {
                tracing::error!("cannot save local preferences: {e}");
            }
        })
        .detach();
        cx.refresh_windows();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensions_fit_small_displays_and_reject_invalid_sizes() {
        let state = WindowState {
            width: f32::NAN,
            height: -2.,
            maximized: true,
        };
        assert_eq!(state.clamped((640., 400.)), (640., 400.));
        assert_eq!(state.clamped((1920., 1080.)), (1180., 760.));
    }
    #[test]
    fn unknown_version_falls_back_without_reusing_backup_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("local-config.json"),
            br#"{"version":99,"backup_directory":"elsewhere"}"#,
        )
        .unwrap();
        let config = LocalConfig::load(dir.path());
        assert_eq!(config.backup_directory, PathBuf::from("backups"));
        assert!(!config.daily_backup);
    }

    #[test]
    fn daily_schedule_waits_a_full_day_and_never_runs_when_disabled() {
        let mut config = LocalConfig {
            daily_backup: true,
            last_automatic: Some(1000),
            ..Default::default()
        };
        assert!(!config.automatic_due(86_400_999));
        assert!(config.automatic_due(86_401_000));
        assert!(!config.automatic_due(0));
        config.daily_backup = false;
        assert!(!config.automatic_due(i64::MAX));
    }

    #[test]
    fn repeated_atomic_save_preserves_local_destination_and_identity() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = LocalConfig::default();
        config.save(dir.path()).unwrap();
        config.backup_directory = "../external-backups".into();
        config.window.maximized = true;
        config.save(dir.path()).unwrap();
        let loaded = LocalConfig::load(dir.path());
        assert_eq!(loaded.collection, config.collection);
        assert_eq!(
            loaded.backup_dir(dir.path()),
            dir.path().join("../external-backups")
        );
        assert!(loaded.window.maximized);
    }
}
