use serde::{Deserialize, Serialize};

/// Default gap between background update checks.
pub const DEFAULT_UPDATE_CHECK_INTERVAL_HOURS: u32 = 24;

/// Smallest accepted interval, so a misconfigured file cannot turn the check
/// into a polling loop against wayscriber.com.
pub const MIN_UPDATE_CHECK_INTERVAL_HOURS: u32 = 1;

/// Update notification preferences.
///
/// Wayscriber never installs anything: the check only compares the running
/// version against the release manifest published on wayscriber.com and points
/// at the update instructions for the user's install method.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatesConfig {
    /// Periodically check whether a newer release exists. Set to `false` (or
    /// export `WAYSCRIBER_DISABLE_UPDATE_CHECK=1`) to never touch the network.
    #[serde(default = "default_true")]
    pub check: bool,

    /// Show a desktop notification the first time a new release is seen.
    /// With this off, the update still appears in the About window and tray.
    #[serde(default = "default_true")]
    pub notify: bool,

    /// Hours between background checks.
    #[serde(default = "default_interval_hours")]
    pub interval_hours: u32,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            check: true,
            notify: true,
            interval_hours: DEFAULT_UPDATE_CHECK_INTERVAL_HOURS,
        }
    }
}

impl UpdatesConfig {
    /// Interval clamped to the supported range.
    pub fn interval(&self) -> std::time::Duration {
        let hours = self.interval_hours.max(MIN_UPDATE_CHECK_INTERVAL_HOURS);
        std::time::Duration::from_secs(u64::from(hours) * 3600)
    }
}

fn default_true() -> bool {
    true
}

fn default_interval_hours() -> u32 {
    DEFAULT_UPDATE_CHECK_INTERVAL_HOURS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_check_daily() {
        let config = UpdatesConfig::default();

        assert!(config.check);
        assert!(config.notify);
        assert_eq!(config.interval(), std::time::Duration::from_secs(86_400));
    }

    #[test]
    fn interval_is_clamped_to_the_minimum() {
        let config = UpdatesConfig {
            interval_hours: 0,
            ..UpdatesConfig::default()
        };

        assert_eq!(config.interval(), std::time::Duration::from_secs(3600));
    }

    #[test]
    fn omitted_keys_fall_back_to_defaults() {
        let partial: UpdatesConfig = toml::from_str("check = false").unwrap();

        assert!(!partial.check);
        assert!(partial.notify);
        assert_eq!(partial.interval_hours, DEFAULT_UPDATE_CHECK_INTERVAL_HOURS);
    }
}
