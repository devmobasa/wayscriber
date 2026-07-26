use super::Config;
use crate::config::types::MIN_UPDATE_CHECK_INTERVAL_HOURS;

/// Longest accepted gap; beyond a month the check stops being useful and is
/// better expressed as `check = false`.
const MAX_INTERVAL_HOURS: u32 = 24 * 30;

impl Config {
    pub(super) fn validate_updates(&mut self) {
        let interval = &mut self.updates.interval_hours;
        if *interval < MIN_UPDATE_CHECK_INTERVAL_HOURS {
            log::warn!(
                "updates.interval_hours {} too small; clamping to {}",
                *interval,
                MIN_UPDATE_CHECK_INTERVAL_HOURS
            );
            *interval = MIN_UPDATE_CHECK_INTERVAL_HOURS;
        }
        if *interval > MAX_INTERVAL_HOURS {
            log::warn!(
                "updates.interval_hours {} too large; clamping to {}",
                *interval,
                MAX_INTERVAL_HOURS
            );
            *interval = MAX_INTERVAL_HOURS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_out_of_range_intervals() {
        let mut config = Config::default();

        config.updates.interval_hours = 0;
        config.validate_updates();
        assert_eq!(
            config.updates.interval_hours,
            MIN_UPDATE_CHECK_INTERVAL_HOURS
        );

        config.updates.interval_hours = 10_000;
        config.validate_updates();
        assert_eq!(config.updates.interval_hours, MAX_INTERVAL_HOURS);
    }

    #[test]
    fn leaves_valid_intervals_alone() {
        let mut config = Config::default();
        config.updates.interval_hours = 12;

        config.validate_updates();

        assert_eq!(config.updates.interval_hours, 12);
    }
}
