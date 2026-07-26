use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CACHE_HOME_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV,
    XDG_PICTURES_DIR_ENV, XDG_RUNTIME_DIR_ENV,
};

/// Environment values captured once for one application root.
#[derive(Clone, Debug, Default)]
pub struct PathEnvironment {
    home: Option<OsString>,
    userprofile: Option<OsString>,
    xdg_config_home: Option<OsString>,
    xdg_cache_home: Option<OsString>,
    xdg_data_home: Option<OsString>,
    xdg_pictures_dir: Option<OsString>,
    xdg_runtime_dir: Option<OsString>,
}

impl PathEnvironment {
    pub fn capture() -> Self {
        Self {
            home: std::env::var_os(HOME_ENV),
            userprofile: std::env::var_os(USERPROFILE_ENV),
            xdg_config_home: std::env::var_os(XDG_CONFIG_HOME_ENV),
            xdg_cache_home: std::env::var_os(XDG_CACHE_HOME_ENV),
            xdg_data_home: std::env::var_os(XDG_DATA_HOME_ENV),
            xdg_pictures_dir: std::env::var_os(XDG_PICTURES_DIR_ENV),
            xdg_runtime_dir: std::env::var_os(XDG_RUNTIME_DIR_ENV),
        }
    }

    /// Build an explicit environment snapshot from selected path variables.
    ///
    /// Unlisted values remain unset. This is useful for embedded callers and
    /// deterministic fixtures that must not mutate process-wide environment.
    pub fn from_values(values: &[(&str, &OsStr)]) -> Self {
        let mut environment = Self::default();
        for (name, value) in values {
            let slot = match *name {
                HOME_ENV => &mut environment.home,
                USERPROFILE_ENV => &mut environment.userprofile,
                XDG_CONFIG_HOME_ENV => &mut environment.xdg_config_home,
                XDG_CACHE_HOME_ENV => &mut environment.xdg_cache_home,
                XDG_DATA_HOME_ENV => &mut environment.xdg_data_home,
                XDG_PICTURES_DIR_ENV => &mut environment.xdg_pictures_dir,
                XDG_RUNTIME_DIR_ENV => &mut environment.xdg_runtime_dir,
                _ => continue,
            };
            *slot = Some((*value).to_os_string());
        }
        environment
    }

    #[cfg(test)]
    pub(crate) fn for_test(values: &[(&str, &OsStr)]) -> Self {
        Self::from_values(values)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathCapability {
    Home,
    Config,
    Cache,
    Data,
    Pictures,
    Runtime,
    Log,
}

impl fmt::Display for PathCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Home => "home",
            Self::Config => "config",
            Self::Cache => "cache",
            Self::Data => "data",
            Self::Pictures => "pictures",
            Self::Runtime => "runtime",
            Self::Log => "log",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathResolutionError {
    RelativeEnvironmentValue {
        variable: &'static str,
        value: PathBuf,
    },
    RelativeUserPath {
        capability: PathCapability,
        value: PathBuf,
    },
    Unavailable {
        capability: PathCapability,
    },
}

impl fmt::Display for PathResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeEnvironmentValue { variable, value } => write!(
                formatter,
                "{variable} must be absolute; received {}",
                value.display()
            ),
            Self::RelativeUserPath { capability, value } => write!(
                formatter,
                "{capability} path must be absolute after supported ~/ expansion; received {}",
                value.display()
            ),
            Self::Unavailable { capability } => {
                write!(formatter, "{capability} path identity is unavailable")
            }
        }
    }
}

impl std::error::Error for PathResolutionError {}

/// Pure path policy for one captured application environment.
#[derive(Clone, Debug)]
pub struct PathResolver {
    environment: PathEnvironment,
}

impl PathResolver {
    pub fn from_environment(environment: PathEnvironment) -> Self {
        Self { environment }
    }

    pub fn from_process_environment() -> Self {
        Self::from_environment(PathEnvironment::capture())
    }

    pub fn home_dir(&self) -> Result<PathBuf, PathResolutionError> {
        match classify_absolute(HOME_ENV, self.environment.home.as_deref())? {
            Some(home) => Ok(home),
            None => classify_absolute(USERPROFILE_ENV, self.environment.userprofile.as_deref())?
                .ok_or(PathResolutionError::Unavailable {
                    capability: PathCapability::Home,
                }),
        }
    }

    pub fn config_dir(&self) -> Result<PathBuf, PathResolutionError> {
        self.xdg_or_home(
            XDG_CONFIG_HOME_ENV,
            self.environment.xdg_config_home.as_deref(),
            PathCapability::Config,
            |home| home.join(".config"),
        )
    }

    pub fn config_file(&self) -> Result<PathBuf, PathResolutionError> {
        self.config_dir()
            .map(|root| root.join("wayscriber").join("config.toml"))
    }

    pub fn cache_dir(&self) -> Result<PathBuf, PathResolutionError> {
        self.xdg_or_home(
            XDG_CACHE_HOME_ENV,
            self.environment.xdg_cache_home.as_deref(),
            PathCapability::Cache,
            |home| home.join(".cache"),
        )
    }

    pub fn update_check_cache_file(&self) -> Result<PathBuf, PathResolutionError> {
        self.cache_dir()
            .map(|root| root.join("wayscriber").join("update-check.json"))
    }

    pub fn data_dir(&self) -> Result<PathBuf, PathResolutionError> {
        self.xdg_or_home(
            XDG_DATA_HOME_ENV,
            self.environment.xdg_data_home.as_deref(),
            PathCapability::Data,
            |home| home.join(".local").join("share"),
        )
    }

    pub fn wayscriber_data_dir(&self) -> Result<PathBuf, PathResolutionError> {
        self.data_dir().map(|root| root.join("wayscriber"))
    }

    pub fn runtime_ui_state_file(&self) -> Result<PathBuf, PathResolutionError> {
        self.wayscriber_data_dir()
            .map(|root| root.join("runtime-ui.toml"))
    }

    pub fn pictures_dir(&self) -> Result<PathBuf, PathResolutionError> {
        if let Some(path) = classify_absolute(
            XDG_PICTURES_DIR_ENV,
            self.environment.xdg_pictures_dir.as_deref(),
        )? {
            return Ok(path);
        }
        self.home_dir()
            .map(|home| home.join("Pictures"))
            .map_err(|_| PathResolutionError::Unavailable {
                capability: PathCapability::Pictures,
            })
    }

    pub fn runtime_dir(&self) -> Result<PathBuf, PathResolutionError> {
        if let Some(path) = classify_absolute(
            XDG_RUNTIME_DIR_ENV,
            self.environment.xdg_runtime_dir.as_deref(),
        )? {
            return Ok(path.join("wayscriber"));
        }
        self.data_dir()
            .map(|data| data.join("wayscriber").join("runtime"))
            .map_err(|_| PathResolutionError::Unavailable {
                capability: PathCapability::Runtime,
            })
    }

    pub fn log_dir(&self) -> Result<PathBuf, PathResolutionError> {
        self.wayscriber_data_dir()
            .map(|data| data.join("logs"))
            .map_err(|_| PathResolutionError::Unavailable {
                capability: PathCapability::Log,
            })
    }

    pub fn require_absolute_user_path(
        &self,
        value: &str,
        capability: PathCapability,
    ) -> Result<PathBuf, PathResolutionError> {
        let path = self.expand_tilde(value)?;
        if path.is_absolute() {
            Ok(path)
        } else {
            Err(PathResolutionError::RelativeUserPath {
                capability,
                value: path,
            })
        }
    }

    pub fn expand_tilde(&self, value: &str) -> Result<PathBuf, PathResolutionError> {
        if let Some(suffix) = value.strip_prefix("~/") {
            return self.home_dir().map(|home| home.join(suffix));
        }
        Ok(PathBuf::from(value))
    }

    fn xdg_or_home(
        &self,
        variable: &'static str,
        value: Option<&OsStr>,
        capability: PathCapability,
        fallback: impl FnOnce(&Path) -> PathBuf,
    ) -> Result<PathBuf, PathResolutionError> {
        if let Some(path) = classify_absolute(variable, value)? {
            return Ok(path);
        }
        self.home_dir()
            .map(|home| fallback(&home))
            .map_err(|_| PathResolutionError::Unavailable { capability })
    }
}

fn classify_absolute(
    variable: &'static str,
    value: Option<&OsStr>,
) -> Result<Option<PathBuf>, PathResolutionError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(Some(path))
    } else {
        Err(PathResolutionError::RelativeEnvironmentValue {
            variable,
            value: path,
        })
    }
}
