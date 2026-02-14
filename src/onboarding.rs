use crate::paths::data_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ONBOARDING_VERSION: u32 = 3;
pub(crate) const DRAWER_HINT_MAX: u32 = 2;
const ONBOARDING_FILE: &str = "onboarding.toml";
const ONBOARDING_DIR: &str = "wayscriber";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FirstRunStep {
    WaitDraw,
    DrawUndo,
    QuickAccess,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub welcome_shown: bool,
    #[serde(default)]
    pub toolbar_hint_shown: bool,
    /// Whether the guided tour has been shown to the user
    #[serde(default, alias = "tour_completed")]
    pub tour_shown: bool,
    /// Whether the "More" drawer hint has been shown
    #[serde(default)]
    pub drawer_hint_shown: bool,
    /// Number of times the drawer hint has been acknowledged (opened)
    #[serde(default)]
    pub drawer_hint_count: u32,
    /// Number of overlay launches seen by this profile
    #[serde(default)]
    pub sessions_seen: u32,
    /// Whether first-run onboarding has been fully completed
    #[serde(default)]
    pub first_run_completed: bool,
    /// Whether the user explicitly skipped first-run onboarding
    #[serde(default)]
    pub first_run_skipped: bool,
    /// Active first-run onboarding step (if any)
    #[serde(default)]
    pub active_step: Option<FirstRunStep>,
    /// Whether quick-access step requires revealing hidden toolbars
    #[serde(default)]
    pub quick_access_requires_toolbar: bool,
    /// Whether radial menu preview has been shown during quick-access step
    #[serde(default)]
    pub quick_access_radial_preview_shown: bool,
    /// Whether context menu preview has been shown during quick-access step
    #[serde(default)]
    pub quick_access_context_preview_shown: bool,
    /// Whether help overlay preview has been shown during reference step
    #[serde(default)]
    pub reference_help_preview_shown: bool,
    /// Whether command palette preview has been shown during reference step
    #[serde(default)]
    pub reference_palette_preview_shown: bool,
    /// Whether at least one stroke was drawn
    #[serde(default)]
    pub first_stroke_done: bool,
    /// Whether at least one successful undo was performed
    #[serde(default)]
    pub first_undo_done: bool,
    /// Whether toolbar visibility was toggled via an action
    #[serde(default)]
    pub used_toolbar_toggle: bool,
    /// Whether radial menu was opened
    #[serde(default)]
    pub used_radial_menu: bool,
    /// Whether context menu was opened by right click
    #[serde(default)]
    pub used_context_menu_right_click: bool,
    /// Whether context menu was opened via keyboard action
    #[serde(default)]
    pub used_context_menu_keyboard: bool,
    /// Whether help overlay was opened
    #[serde(default)]
    pub used_help_overlay: bool,
    /// Whether command palette was opened
    #[serde(default)]
    pub used_command_palette: bool,
    /// Whether deferred help hint has already been shown
    #[serde(default)]
    pub hint_help_shown: bool,
    /// Whether deferred command palette hint has already been shown
    #[serde(default)]
    pub hint_palette_shown: bool,
    /// Whether deferred quick-access hint has already been shown
    #[serde(default)]
    pub hint_quick_access_shown: bool,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            version: ONBOARDING_VERSION,
            welcome_shown: false,
            toolbar_hint_shown: false,
            tour_shown: false,
            drawer_hint_shown: false,
            drawer_hint_count: 0,
            sessions_seen: 0,
            first_run_completed: false,
            first_run_skipped: false,
            active_step: None,
            quick_access_requires_toolbar: false,
            quick_access_radial_preview_shown: false,
            quick_access_context_preview_shown: false,
            reference_help_preview_shown: false,
            reference_palette_preview_shown: false,
            first_stroke_done: false,
            first_undo_done: false,
            used_toolbar_toggle: false,
            used_radial_menu: false,
            used_context_menu_right_click: false,
            used_context_menu_keyboard: false,
            used_help_overlay: false,
            used_command_palette: false,
            hint_help_shown: false,
            hint_palette_shown: false,
            hint_quick_access_shown: false,
        }
    }
}

impl OnboardingState {
    pub fn first_run_active(&self) -> bool {
        !self.first_run_completed && !self.first_run_skipped
    }
}

pub struct OnboardingStore {
    state: OnboardingState,
    path: Option<PathBuf>,
}

impl OnboardingStore {
    pub fn load() -> Self {
        let Some(path) = onboarding_path() else {
            return Self {
                state: OnboardingState::default(),
                path: None,
            };
        };

        Self::load_from_path(path)
    }

    fn load_from_path(path: PathBuf) -> Self {
        match fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<OnboardingState>(&raw) {
                Ok(mut state) => {
                    let needs_save = migrate_onboarding_state(&mut state);
                    let store = Self {
                        state,
                        path: Some(path),
                    };
                    if needs_save {
                        store.save();
                    }
                    return store;
                }
                Err(err) => {
                    warn!(
                        "Failed to parse onboarding state {}: {}",
                        path.display(),
                        err
                    );
                    let state = recover_onboarding_file(&path, Some(&raw));
                    return Self {
                        state,
                        path: Some(path),
                    };
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                warn!(
                    "Failed to read onboarding state {}: {}",
                    path.display(),
                    err
                );
                let state = recover_onboarding_file(&path, None);
                return Self {
                    state,
                    path: Some(path),
                };
            }
        }

        Self {
            state: OnboardingState::default(),
            path: Some(path),
        }
    }

    pub fn state(&self) -> &OnboardingState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut OnboardingState {
        &mut self.state
    }

    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            warn!(
                "Failed to create onboarding state dir {}: {}",
                parent.display(),
                err
            );
            return;
        }
        match toml::to_string_pretty(&self.state) {
            Ok(contents) => {
                if let Err(err) = fs::write(path, contents) {
                    warn!(
                        "Failed to write onboarding state {}: {}",
                        path.display(),
                        err
                    );
                }
            }
            Err(err) => {
                warn!("Failed to serialize onboarding state: {}", err);
            }
        }
    }
}

fn onboarding_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(ONBOARDING_DIR).join(ONBOARDING_FILE))
}

fn default_version() -> u32 {
    ONBOARDING_VERSION
}

fn migrate_onboarding_state(state: &mut OnboardingState) -> bool {
    let mut needs_save = false;
    let old_version = state.version;

    if state.version != ONBOARDING_VERSION {
        state.version = ONBOARDING_VERSION;
        needs_save = true;
    }
    if state.drawer_hint_count == 0 && state.drawer_hint_shown {
        state.drawer_hint_count = DRAWER_HINT_MAX;
        needs_save = true;
    }
    if state.drawer_hint_count >= DRAWER_HINT_MAX && !state.drawer_hint_shown {
        state.drawer_hint_shown = true;
        needs_save = true;
    }

    // Existing users already saw onboarding in earlier versions; don't force re-run.
    if old_version < 3 && !state.first_run_completed && (state.welcome_shown || state.tour_shown) {
        state.first_run_completed = true;
        state.first_run_skipped = false;
        state.active_step = None;
        needs_save = true;
    }

    if state.first_run_skipped && !state.first_run_completed {
        state.first_run_completed = true;
        needs_save = true;
    }
    if state.first_run_completed && state.active_step.is_some() {
        state.active_step = None;
        needs_save = true;
    }
    if state.quick_access_requires_toolbar && state.active_step != Some(FirstRunStep::QuickAccess) {
        state.quick_access_requires_toolbar = false;
        needs_save = true;
    }

    needs_save
}

fn recover_onboarding_file(path: &Path, _raw: Option<&str>) -> OnboardingState {
    if path.exists() {
        let backup = backup_path(path);
        if let Err(err) = fs::rename(path, &backup) {
            warn!(
                "Failed to back up onboarding state {}: {}",
                path.display(),
                err
            );
        }
    }

    let welcome_shown = true;
    let toolbar_hint_shown = true;

    let state = OnboardingState {
        version: ONBOARDING_VERSION,
        welcome_shown,
        toolbar_hint_shown,
        tour_shown: true,        // Don't show legacy tour for recovered state
        drawer_hint_shown: true, // Don't show drawer hint for recovered state
        drawer_hint_count: DRAWER_HINT_MAX,
        sessions_seen: 0,
        first_run_completed: true,
        first_run_skipped: false,
        active_step: None,
        quick_access_requires_toolbar: false,
        quick_access_radial_preview_shown: false,
        quick_access_context_preview_shown: false,
        reference_help_preview_shown: false,
        reference_palette_preview_shown: false,
        first_stroke_done: false,
        first_undo_done: false,
        used_toolbar_toggle: false,
        used_radial_menu: false,
        used_context_menu_right_click: false,
        used_context_menu_keyboard: false,
        used_help_overlay: false,
        used_command_palette: false,
        hint_help_shown: true,
        hint_palette_shown: true,
        hint_quick_access_shown: true,
    };
    let store = OnboardingStore {
        state: state.clone(),
        path: Some(path.to_path_buf()),
    };
    store.save();
    state
}

fn backup_path(path: &Path) -> PathBuf {
    let base = path.with_extension("bak");
    if !base.exists() {
        return base;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let mut candidate = path.with_extension(format!("bak.{nanos}.{pid}"));
    if !candidate.exists() {
        return candidate;
    }
    for index in 1..=1000 {
        candidate = path.with_extension(format!("bak.{nanos}.{pid}.{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn onboarding_defaults_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir should succeed");
        let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
        let store = OnboardingStore::load_from_path(path.clone());
        assert!(!store.state().welcome_shown);
        assert!(!store.state().toolbar_hint_shown);
        assert!(!store.state().first_run_completed);
        assert!(store.state().active_step.is_none());

        store.save();
        assert!(path.exists());
    }

    #[test]
    fn onboarding_persists_flags() {
        let tmp = tempfile::tempdir().expect("tempdir should succeed");
        let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
        let mut store = OnboardingStore::load_from_path(path.clone());
        store.state_mut().welcome_shown = true;
        store.state_mut().toolbar_hint_shown = true;
        store.state_mut().used_help_overlay = true;
        store.save();

        let reloaded = OnboardingStore::load_from_path(path.clone());
        assert!(reloaded.state().welcome_shown);
        assert!(reloaded.state().toolbar_hint_shown);
        assert!(reloaded.state().used_help_overlay);
    }

    #[test]
    fn onboarding_recovers_from_parse_error() {
        let tmp = tempfile::tempdir().expect("tempdir should succeed");
        let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create onboarding dir");
        }
        fs::write(&path, "not = [toml").expect("write invalid toml");

        let store = OnboardingStore::load_from_path(path.clone());
        assert!(store.state().welcome_shown);
        assert!(store.state().first_run_completed);
        assert!(path.exists());

        let backup_found = fs::read_dir(path.parent().expect("parent dir"))
            .expect("read onboarding dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("onboarding.bak")
            });
        assert!(backup_found);

        let contents = fs::read_to_string(&path).expect("read recovered file");
        let state: OnboardingState =
            toml::from_str(&contents).expect("recovered file should parse");
        assert!(state.welcome_shown);
        assert!(state.first_run_completed);
    }

    #[test]
    fn onboarding_version_bump_saves() {
        let tmp = tempfile::tempdir().expect("tempdir should succeed");
        let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create onboarding dir");
        }
        let seed = "version = 0\nwelcome_shown = true\ntoolbar_hint_shown = false\n";
        fs::write(&path, seed).expect("write seed");

        let store = OnboardingStore::load_from_path(path.clone());
        assert!(store.state().welcome_shown);
        assert_eq!(store.state().version, ONBOARDING_VERSION);
        assert!(store.state().first_run_completed);

        let contents = fs::read_to_string(&path).expect("read bumped file");
        let state: OnboardingState = toml::from_str(&contents).expect("bumped file should parse");
        assert_eq!(state.version, ONBOARDING_VERSION);
        assert!(state.welcome_shown);
        assert!(state.first_run_completed);
    }
}
