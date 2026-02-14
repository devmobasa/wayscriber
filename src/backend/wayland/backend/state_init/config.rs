use log::{debug, info, warn};

use crate::backend::ExitAfterCaptureMode;
use crate::config::{Config, ConfigSource};

pub(super) struct LoadedConfig {
    pub(super) config: Config,
    pub(super) source: ConfigSource,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
}

pub(super) fn load(backend_exit_mode: ExitAfterCaptureMode) -> LoadedConfig {
    let (config, source) = match Config::load() {
        Ok(loaded) => (loaded.config, loaded.source),
        Err(e) => {
            warn!("Failed to load config: {}. Using defaults.", e);
            (Config::default(), ConfigSource::Default)
        }
    };
    let exit_after_capture_mode = match backend_exit_mode {
        ExitAfterCaptureMode::Auto if config.capture.exit_after_capture => {
            ExitAfterCaptureMode::Always
        }
        other => other,
    };

    info!("Configuration loaded");
    log_config(&config);

    LoadedConfig {
        config,
        source,
        exit_after_capture_mode,
    }
}

fn log_config(config: &Config) {
    debug!("  Color: {:?}", config.drawing.default_color);
    debug!("  Thickness: {:.1}px", config.drawing.default_thickness);
    debug!("  Font size: {:.1}px", config.drawing.default_font_size);
    debug!("  Buffer count: {}", config.performance.buffer_count);
    debug!("  VSync: {}", config.performance.enable_vsync);
    debug!(
        "  Status bar: {} @ {:?}",
        config.ui.show_status_bar, config.ui.status_bar_position
    );
    debug!(
        "  Status bar font size: {}",
        config.ui.status_bar_style.font_size
    );
    debug!(
        "  Help overlay font size: {}",
        config.ui.help_overlay_style.font_size
    );
    #[cfg(tablet)]
    info!(
        "Tablet feature: compiled=yes, runtime_enabled={}",
        config.tablet.enabled
    );
    #[cfg(not(tablet))]
    info!("Tablet feature: compiled=no");
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::config::test_helpers::with_temp_config_home;

    #[test]
    fn load_applies_capture_exit_after_capture_to_auto_mode() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(path, "[capture]\nexit_after_capture = true\n").expect("write config");

            let loaded = load(ExitAfterCaptureMode::Auto);
            assert!(matches!(loaded.source, ConfigSource::Primary));
            assert!(matches!(
                loaded.exit_after_capture_mode,
                ExitAfterCaptureMode::Always
            ));
        });
    }

    #[test]
    fn load_falls_back_to_defaults_when_config_is_invalid() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(path, "not = [valid").expect("write invalid config");

            let loaded = load(ExitAfterCaptureMode::Auto);
            assert!(matches!(loaded.source, ConfigSource::Default));
            assert!(matches!(
                loaded.exit_after_capture_mode,
                ExitAfterCaptureMode::Auto
            ));
            assert!(!loaded.config.capture.exit_after_capture);
        });
    }
}
