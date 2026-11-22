//! Device/storage preferences, deliberately independent of the restored database.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Global};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// Normal window size and position in logical pixels, plus its last maximized state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct WindowState {
    pub width: f32,
    pub height: f32,
    /// Top-left corner in global display coordinates; `None` until first moved
    /// or resized (the window then opens centered).
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1180.,
            height: 760.,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl WindowState {
    /// Where to put a window of `size` so it reopens where it was, or `None` to
    /// center it: when no position is saved, or the saved one is no longer on a
    /// connected display (`displays` are their visible areas as x, y, width, height).
    pub fn origin(
        &self,
        size: (f32, f32),
        displays: &[(f32, f32, f32, f32)],
    ) -> Option<(f32, f32)> {
        let (x, y) = (self.x?, self.y?);
        let (dx, dy, dw, dh) = self.display_at(size.0, displays)?;
        // Keep the whole window on that display when it fits.
        let x = x.min(dx + dw - size.0).max(dx);
        let y = y.min(dy + dh - size.1).max(dy);
        Some((x, y))
    }

    /// The display the window was last on, if it is still connected.
    pub fn display(&self, displays: &[(f32, f32, f32, f32)]) -> Option<(f32, f32, f32, f32)> {
        self.display_at(self.width, displays)
    }

    /// The display under the middle of the title bar of a window `width` wide at
    /// the saved position: the title bar must stay reachable to move the window.
    fn display_at(
        &self,
        width: f32,
        displays: &[(f32, f32, f32, f32)],
    ) -> Option<(f32, f32, f32, f32)> {
        let (x, y) = (self.x?, self.y?);
        if !x.is_finite() || !y.is_finite() || !width.is_finite() {
            return None;
        }
        let (cx, cy) = (x + width / 2., y + 16.);
        displays
            .iter()
            .copied()
            .find(|(dx, dy, dw, dh)| cx >= *dx && cx < dx + dw && cy >= *dy && cy < dy + dh)
    }

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
            // On by default: losing notes costs far more than a few backups on disk
            // (`keep_automatic` bounds how many are kept).
            daily_backup: true,
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
            ..Default::default()
        };
        assert_eq!(state.clamped((640., 400.)), (640., 400.));
        assert_eq!(state.clamped((1920., 1080.)), (1180., 760.));
    }

    #[test]
    fn position_is_restored_only_on_a_connected_display() {
        let primary = (0., 0., 2560., 1032.);
        let second = (2560., 0., 1920., 1080.);
        let at = |x, y| WindowState {
            x: Some(x),
            y: Some(y),
            ..Default::default()
        };
        let size = (1000., 700.);
        // Never moved: centered.
        assert_eq!(WindowState::default().origin(size, &[primary]), None);
        // On the second monitor, while it is connected.
        assert_eq!(
            at(3000., 100.).origin(size, &[primary, second]),
            Some((3000., 100.))
        );
        // The second monitor was unplugged: centered on what is left.
        assert_eq!(at(3000., 100.).origin(size, &[primary]), None);
        // Partly off the bottom-right edge: pulled back inside the display.
        assert_eq!(
            at(2000., 900.).origin(size, &[primary]),
            Some((1560., 332.))
        );
        assert_eq!(at(f32::NAN, 0.).origin(size, &[primary]), None);
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
        assert_eq!(config.daily_backup, LocalConfig::default().daily_backup);
    }

    #[test]
    fn saved_preferences_keep_daily_backups_off() {
        // Configs from earlier versions store the field, so the new default does not
        // turn the schedule on for people who left it off.
        let dir = tempfile::tempdir().unwrap();
        let config = LocalConfig {
            daily_backup: false,
            ..Default::default()
        };
        config.save(dir.path()).unwrap();
        assert!(!LocalConfig::load(dir.path()).daily_backup);
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
