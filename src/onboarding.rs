use crate::paths::data_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

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
        let path = onboarding_path();
        if let Some(path) = &path {
            match fs::read_to_string(path) {
                Ok(raw) => match toml::from_str::<OnboardingState>(&raw) {
                    Ok(mut state) => {
                        if state.version != ONBOARDING_VERSION {
                            state.version = ONBOARDING_VERSION;
                        }
                        return Self {
                            state,
                            path: Some(path.clone()),
                        };
                    }
                    Err(err) => {
                        warn!(
                            "Failed to parse onboarding state {}: {}",
                            path.display(),
                            err
                        );
                    }
                },
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    warn!(
                        "Failed to read onboarding state {}: {}",
                        path.display(),
                        err
                    );
                }
            }
        }

        Self {
            state: OnboardingState::default(),
            path,
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
