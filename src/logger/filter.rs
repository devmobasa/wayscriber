use std::env;

use log::{Level, LevelFilter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LogFilter {
    default: LevelFilter,
    directives: Vec<LogDirective>,
    max_level: LevelFilter,
}

impl LogFilter {
    pub(super) fn from_env() -> Self {
        match env::var("RUST_LOG") {
            Ok(value) => Self::parse(&value, LevelFilter::Off),
            Err(_) => Self::parse("info", LevelFilter::Info),
        }
    }

    fn parse(value: &str, fallback: LevelFilter) -> Self {
        let mut default = fallback;
        let mut directives = Vec::new();
        let mut accepted_filter = false;

        for raw_directive in value.split(',') {
            let directive = raw_directive.trim();
            if directive.is_empty() {
                continue;
            }

            if let Some((target, level)) = directive.split_once('=') {
                let target = target.trim();
                if target.is_empty() {
                    continue;
                }
                if let Some(level) = parse_level(level.trim()) {
                    directives.push(LogDirective {
                        target: target.to_string(),
                        level,
                    });
                    accepted_filter = true;
                }
            } else if let Some(level) = parse_level(directive) {
                default = level;
                accepted_filter = true;
            } else {
                directives.push(LogDirective {
                    target: directive.to_string(),
                    level: LevelFilter::Trace,
                });
                accepted_filter = true;
            }
        }

        if !accepted_filter && fallback == LevelFilter::Off {
            default = LevelFilter::Error;
        }

        let max_level = directives
            .iter()
            .map(|directive| directive.level)
            .fold(default, max_level_filter);
        Self {
            default,
            directives,
            max_level,
        }
    }

    pub(super) fn max_level(&self) -> LevelFilter {
        self.max_level
    }

    pub(super) fn enabled(&self, target: &str, level: Level) -> bool {
        level.to_level_filter() <= self.level_for(target)
    }

    fn level_for(&self, target: &str) -> LevelFilter {
        let mut matched = None;
        for directive in &self.directives {
            if target_matches(&directive.target, target) {
                let target_len = directive.target.len();
                if matched.is_none_or(|(matched_len, _)| target_len >= matched_len) {
                    matched = Some((target_len, directive.level));
                }
            }
        }
        matched.map_or(self.default, |(_, level)| level)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LogDirective {
    target: String,
    level: LevelFilter,
}

fn parse_level(value: &str) -> Option<LevelFilter> {
    match value.to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn target_matches(directive: &str, target: &str) -> bool {
    target == directive
        || target
            .strip_prefix(directive)
            .is_some_and(|remaining| remaining.starts_with("::"))
}

fn max_level_filter(left: LevelFilter, right: LevelFilter) -> LevelFilter {
    if left >= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use super::{LogFilter, max_level_filter};
    use log::{Level, LevelFilter};

    #[test]
    fn missing_rust_log_defaults_to_info() {
        let filter = LogFilter::parse("info", LevelFilter::Info);

        assert!(filter.enabled("wayscriber", Level::Info));
        assert!(!filter.enabled("wayscriber", Level::Debug));
        assert_eq!(filter.max_level(), LevelFilter::Info);
    }

    #[test]
    fn global_rust_log_level_applies_to_all_targets() {
        let filter = LogFilter::parse("debug", LevelFilter::Off);

        assert!(filter.enabled("wayscriber", Level::Debug));
        assert!(filter.enabled("zbus", Level::Debug));
        assert!(!filter.enabled("wayscriber", Level::Trace));
        assert_eq!(filter.max_level(), LevelFilter::Debug);
    }

    #[test]
    fn empty_rust_log_falls_back_to_error() {
        let filter = LogFilter::parse("  , ", LevelFilter::Off);

        assert!(filter.enabled("wayscriber", Level::Error));
        assert!(!filter.enabled("wayscriber", Level::Warn));
        assert_eq!(filter.max_level(), LevelFilter::Error);
    }

    #[test]
    fn unusable_rust_log_directives_fall_back_to_error() {
        let filter = LogFilter::parse("wayscriber=verbose,zbus=", LevelFilter::Off);

        assert!(filter.enabled("wayscriber", Level::Error));
        assert!(!filter.enabled("wayscriber", Level::Warn));
        assert_eq!(filter.max_level(), LevelFilter::Error);
    }

    #[test]
    fn explicit_off_rust_log_stays_silent() {
        let filter = LogFilter::parse("off", LevelFilter::Off);

        assert!(!filter.enabled("wayscriber", Level::Error));
        assert_eq!(filter.max_level(), LevelFilter::Off);
    }

    #[test]
    fn target_directives_match_module_boundaries() {
        let filter = LogFilter::parse("warn,wayscriber=debug,zbus=off", LevelFilter::Off);

        assert!(filter.enabled("wayscriber::daemon", Level::Debug));
        assert!(filter.enabled("wayscriber_extra", Level::Warn));
        assert!(!filter.enabled("wayscriber_extra", Level::Info));
        assert!(!filter.enabled("zbus", Level::Error));
        assert_eq!(filter.max_level(), LevelFilter::Debug);
    }

    #[test]
    fn later_target_directives_override_earlier_ones() {
        let filter = LogFilter::parse("wayscriber=trace,wayscriber=error", LevelFilter::Off);

        assert!(filter.enabled("wayscriber", Level::Error));
        assert!(!filter.enabled("wayscriber", Level::Warn));
        assert_eq!(filter.max_level(), LevelFilter::Trace);
    }

    #[test]
    fn more_specific_target_directives_override_broader_later_matches() {
        let filter = LogFilter::parse("wayscriber::daemon=debug,wayscriber=info", LevelFilter::Off);

        assert!(filter.enabled("wayscriber::daemon", Level::Debug));
        assert!(!filter.enabled("wayscriber::daemon", Level::Trace));
        assert!(filter.enabled("wayscriber::ui", Level::Info));
        assert!(!filter.enabled("wayscriber::ui", Level::Debug));
        assert_eq!(filter.max_level(), LevelFilter::Debug);
    }

    #[test]
    fn bare_target_directive_enables_trace_for_that_target() {
        let filter = LogFilter::parse("wayscriber", LevelFilter::Off);

        assert!(filter.enabled("wayscriber::backend", Level::Trace));
        assert!(!filter.enabled("zbus", Level::Error));
        assert_eq!(filter.max_level(), LevelFilter::Trace);
    }

    #[test]
    fn max_level_filter_returns_more_verbose_level() {
        assert_eq!(
            max_level_filter(LevelFilter::Warn, LevelFilter::Debug),
            LevelFilter::Debug
        );
    }
}
