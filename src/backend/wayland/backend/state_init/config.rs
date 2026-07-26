use crate::backend::ExitAfterCaptureMode;
use crate::config::{Config, ConfigSource};

pub(super) struct LoadedConfig {
    pub(super) config: Config,
    #[cfg(test)]
    pub(super) source: ConfigSource,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
}

pub(super) fn load(
    backend_exit_mode: ExitAfterCaptureMode,
    config_store: &crate::config::ConfigStore,
    logger: &crate::logger::LoggerHandle,
) -> LoadedConfig {
    let (config, source) = match config_store.load() {
        Ok(loaded) => (loaded.config, loaded.source),
        Err(e) => {
            logger.warn(
                "wayscriber::config",
                format!("Failed to load config: {e}. Using defaults."),
            );
            (Config::default(), ConfigSource::Default)
        }
    };

    let exit_after_capture_mode = match backend_exit_mode {
        ExitAfterCaptureMode::Auto if config.capture.exit_after_capture => {
            ExitAfterCaptureMode::Always
        }
        other => other,
    };

    logger.info(
        "wayscriber::config",
        format!("Configuration loaded from {source:?}"),
    );
    log_config(&config, logger);

    LoadedConfig {
        config,
        #[cfg(test)]
        source,
        exit_after_capture_mode,
    }
}

fn log_config(config: &Config, logger: &crate::logger::LoggerHandle) {
    let target = "wayscriber::config";
    logger.debug(target, format!("  Theme: {:?}", config.ui.theme));
    logger.debug(
        target,
        format!("  Reduced motion: {:?}", config.ui.reduced_motion),
    );
    logger.debug(
        target,
        format!("  Color: {:?}", config.drawing.default_color),
    );
    logger.debug(
        target,
        format!("  Thickness: {:.1}px", config.drawing.default_thickness),
    );
    logger.debug(
        target,
        format!("  Font size: {:.1}px", config.drawing.default_font_size),
    );
    logger.debug(
        target,
        format!("  Buffer count: {}", config.performance.buffer_count),
    );
    logger.debug(
        target,
        format!("  VSync: {}", config.performance.enable_vsync),
    );
    logger.debug(
        target,
        format!(
            "  Status bar: {} @ {:?}",
            config.ui.show_status_bar, config.ui.status_bar_position
        ),
    );
    logger.debug(
        target,
        format!(
            "  Status bar font size: {}",
            config.ui.status_bar_style.font_size
        ),
    );
    logger.debug(
        target,
        format!(
            "  Help overlay font size: {}",
            config.ui.help_overlay_style.font_size
        ),
    );
    #[cfg(feature = "tablet-input")]
    logger.info(
        target,
        format!(
            "Tablet feature: compiled=yes, runtime_enabled={}",
            config.tablet.enabled
        ),
    );
    #[cfg(not(feature = "tablet-input"))]
    logger.info(target, "Tablet feature: compiled=no");
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::config::test_helpers::{test_config_store, with_temp_config_home};

    #[test]
    fn load_applies_capture_exit_after_capture_to_auto_mode() {
        with_temp_config_home(|config_root| {
            let store = test_config_store(config_root);
            let path = store.config_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(path, "[capture]\nexit_after_capture = true\n").expect("write config");

            let loaded = load(
                ExitAfterCaptureMode::Auto,
                &store,
                &crate::logger::LoggerHandle::discarding(),
            );
            assert!(matches!(loaded.source, ConfigSource::Primary));
            assert!(matches!(
                loaded.exit_after_capture_mode,
                ExitAfterCaptureMode::Always
            ));
        });
    }

    #[test]
    fn load_falls_back_to_defaults_when_config_is_invalid() {
        with_temp_config_home(|config_root| {
            let store = test_config_store(config_root);
            let path = store.config_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(path, "not = [valid").expect("write invalid config");

            let loaded = load(
                ExitAfterCaptureMode::Auto,
                &store,
                &crate::logger::LoggerHandle::discarding(),
            );
            assert!(matches!(loaded.source, ConfigSource::Default));
            assert!(matches!(
                loaded.exit_after_capture_mode,
                ExitAfterCaptureMode::Auto
            ));
            assert!(!loaded.config.capture.exit_after_capture);
        });
    }
}
