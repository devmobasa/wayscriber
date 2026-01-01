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
