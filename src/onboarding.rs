use crate::paths::data_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ONBOARDING_VERSION: u32 = 1;
const ONBOARDING_FILE: &str = "onboarding.toml";
const ONBOARDING_DIR: &str = "wayscriber";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub welcome_shown: bool,
    #[serde(default)]
    pub toolbar_hint_shown: bool,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            version: ONBOARDING_VERSION,
            welcome_shown: false,
            toolbar_hint_shown: false,
        }
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
                    let mut needs_save = false;
                    if state.version != ONBOARDING_VERSION {
                        state.version = ONBOARDING_VERSION;
                        needs_save = true;
                    }
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
        store.save();

        let reloaded = OnboardingStore::load_from_path(path.clone());
        assert!(reloaded.state().welcome_shown);
        assert!(reloaded.state().toolbar_hint_shown);
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

        let contents = fs::read_to_string(&path).expect("read bumped file");
        let state: OnboardingState = toml::from_str(&contents).expect("bumped file should parse");
        assert_eq!(state.version, ONBOARDING_VERSION);
        assert!(state.welcome_shown);
    }
}
