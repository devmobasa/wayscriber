use log::{debug, info, warn};

use crate::backend::ExitAfterCaptureMode;
use crate::config::{Config, ConfigSource};

pub(super) struct LoadedConfig {
    pub(super) config: Config,
    pub(super) source: ConfigSource,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
}

pub(super) fn load(backend_exit_mode: ExitAfterCaptureMode) -> LoadedConfig {
    persist_pending_migrations();

    let (config, source) = match Config::load() {
        Ok(loaded) => (loaded.config, loaded.source),
        Err(e) => {
            warn!("Failed to load config: {}. Using defaults.", e);
            (Config::default(), ConfigSource::Default)
        }
    };

    // Install process-wide UI preferences before any surface renders. The
    // daemon spawns fresh overlay processes that re-enter this load path, so
    // this single call site covers both direct and daemon-managed overlays.
    crate::ui::theme::init(config.ui.theme.to_theme_mode());
    crate::ui::anim::set_motion_enabled(config.ui.reduced_motion.motion_enabled());

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

/// Records one-time config migrations on disk before the overlay reads them.
///
/// The overlay owns this write: the daemon, tray, and configurator load the
/// same file and must not race one another for a rewrite the user did not ask
/// for. A failure (read-only file, a lost revision race) leaves the migration
/// in memory only and is retried on the next launch.
fn persist_pending_migrations() {
    if let Err(error) = Config::persist_pending_migrations() {
        warn!("Failed to persist config migrations: {error:#}. Continuing without them.");
    }
}

fn log_config(config: &Config) {
    debug!("  Theme: {:?}", config.ui.theme);
    debug!("  Reduced motion: {:?}", config.ui.reduced_motion);
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
    #[cfg(feature = "tablet-input")]
    info!(
        "Tablet feature: compiled=yes, runtime_enabled={}",
        config.tablet.enabled
    );
    #[cfg(not(feature = "tablet-input"))]
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
    fn load_records_pending_migrations_in_the_config_file() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(
                &path,
                "[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\nundo = [\"Ctrl+Alt+U\"]\n",
            )
            .expect("write legacy config");

            let loaded = load(ExitAfterCaptureMode::Auto);
            assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F9"]);

            let saved = fs::read_to_string(&path).expect("read migrated config");
            assert!(saved.contains(&format!(
                "config_revision = {}",
                crate::config::CURRENT_CONFIG_REVISION
            )));
            assert!(saved.contains("toggle_toolbar = [\"F9\"]"));
            assert!(saved.contains("undo = [\"Ctrl+Alt+U\"]"));
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
