use log::{debug, info, warn};

use crate::backend::ExitAfterCaptureMode;
use crate::config::{
    Action, Config, ConfigSectionError, ConfigSource, ConfigValidationReport,
    DefaultShortcutSkipped, InvalidKeybinding, KeybindingConflictResolution,
};
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};
use crate::notification;

/// How long the keybinding warnings stay up. All of them are on the long side:
/// they describe a config problem the user has to leave the overlay to fix.
const KEYBINDING_CONFLICT_TOAST_MS: u64 = 20_000;
const KEYBINDING_CONFLICT_NOTIFICATION_TIMEOUT_MS: i32 = 20_000;
/// Entries spelled out in a desktop notification before it starts counting.
const KEYBINDING_CONFLICT_NOTIFICATION_LIMIT: usize = 5;

pub(super) struct LoadedConfig {
    pub(super) config: Config,
    pub(super) source: ConfigSource,
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,
    /// Shortcut strings this load had to drop, and duplicates it had to
    /// resolve, in memory.
    pub(super) keybindings: ConfigValidationReport,
    /// What of the authored file this session is not running on.
    pub(super) load_failure: Option<ConfigLoadFailure>,
}

/// Authored configuration this load had to replace with defaults. Reported to
/// the user like the keybinding problems below: a `log::warn` alone hid the
/// total-loss variant of this for as long as it hid #293.
pub(super) enum ConfigLoadFailure {
    /// The whole file was unreadable (I/O or TOML syntax); every setting is a
    /// default.
    File { error: String },
    /// Named top-level sections were unreadable; those sections are defaults,
    /// the rest of the file is in effect.
    Sections(Vec<ConfigSectionError>),
}

impl ConfigLoadFailure {
    /// Whether `section` is running on defaults because authored config could
    /// not be read. A whole-file failure necessarily includes every section.
    pub(super) fn section_failed(&self, section: &str) -> bool {
        match self {
            Self::File { .. } => true,
            Self::Sections(sections) => sections.iter().any(|entry| entry.section == section),
        }
    }
}

/// Reads the configuration. Nothing here writes it: `config.toml` is an
/// authored input for the life of this process, so an old `config_revision`, a
/// mistyped shortcut, or a contested key is reported to the user instead of
/// being repaired behind their back.
pub(super) fn load(backend_exit_mode: ExitAfterCaptureMode) -> LoadedConfig {
    let (config, source, keybindings, load_failure) = match Config::load() {
        Ok(loaded) => {
            let load_failure = if loaded.section_errors.is_empty() {
                None
            } else {
                warn!(
                    "Config sections could not be read and are using defaults: {}",
                    loaded
                        .section_errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                );
                Some(ConfigLoadFailure::Sections(loaded.section_errors))
            };
            (
                loaded.config,
                loaded.source,
                loaded.validation,
                load_failure,
            )
        }
        Err(e) => {
            warn!("Failed to load config: {}. Using defaults.", e);
            (
                Config::default(),
                ConfigSource::Default,
                ConfigValidationReport::default(),
                Some(ConfigLoadFailure::File {
                    error: format!("{e:#}"),
                }),
            )
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
        keybindings,
        load_failure,
    }
}

/// Tells the user which part of their configuration this session is not
/// running on.
///
/// Loading never repairs the file, so a bad value keeps coming back at every
/// launch until the user fixes it — and they can only do that if startup says
/// so. Before this, a single mistyped value threw away every customization in
/// the file with nothing but a log line.
pub(super) fn notify_config_load_failure(
    input_state: &mut InputState,
    tokio_handle: &tokio::runtime::Handle,
    failure: Option<&ConfigLoadFailure>,
) {
    let Some(failure) = failure else {
        return;
    };

    let (toast_text, notification_body) = match failure {
        ConfigLoadFailure::File { error } => (
            "Config file could not be read; all settings are using defaults".to_string(),
            format!(
                "{error}\nNothing was changed in {}; fix the file to get your settings back.",
                config_path_display()
            ),
        ),
        ConfigLoadFailure::Sections(sections) => {
            let names = sections
                .iter()
                .map(|entry| format!("[{}]", entry.section))
                .collect::<Vec<_>>()
                .join(", ");
            let details = sections
                .iter()
                .map(|entry| format!("• {entry}."))
                .collect::<Vec<_>>()
                .join("\n");
            (
                format!("Config {names} could not be read; using defaults for it"),
                format!(
                    "{details}\nNothing was changed in {}; fix the value there to get the section back.",
                    config_path_display()
                ),
            )
        }
    };

    input_state.push_toast(
        ToastPriority::Action,
        "config.load-failure",
        Toast::warning(toast_text)
            .action("Settings", Action::OpenConfigurator)
            .duration_ms(KEYBINDING_CONFLICT_TOAST_MS),
    );
    notification::send_notification_with_timeout_async(
        tokio_handle,
        "Configuration Not Loaded".to_string(),
        notification_body,
        Some("dialog-warning".to_string()),
        KEYBINDING_CONFLICT_NOTIFICATION_TIMEOUT_MS,
    );
}

/// Tells the user which shortcut collided and which action lost it.
///
/// Loading resolves a duplicate shortcut per binding and never writes the
/// result back, so without this the config file keeps a conflict while a
/// shortcut silently stops working — the shape of #293, which a `log::warn`
/// alone hid for weeks. The toast covers the running overlay and the desktop
/// notification covers a launch the user is not looking at.
pub(super) fn notify_keybinding_conflicts(
    input_state: &mut InputState,
    tokio_handle: &tokio::runtime::Handle,
    conflicts: &[KeybindingConflictResolution],
) {
    if conflicts.is_empty() {
        return;
    }

    input_state.push_toast(
        ToastPriority::Action,
        "keybindings.conflict",
        Toast::warning(keybinding_conflict_toast(conflicts))
            .action("Shortcuts", Action::OpenConfiguratorKeybindings)
            .duration_ms(KEYBINDING_CONFLICT_TOAST_MS),
    );
    notification::send_notification_with_timeout_async(
        tokio_handle,
        "Conflicting Shortcuts".to_string(),
        keybinding_conflict_notification_body(conflicts, &config_path_display()),
        Some("dialog-warning".to_string()),
        KEYBINDING_CONFLICT_NOTIFICATION_TIMEOUT_MS,
    );
}

/// Tells the user which shortcut strings the parser rejected.
///
/// A typo used to cost the session every other shortcut too, because the whole
/// keymap failed and the runtime fell back to the shipped defaults. Loading now
/// drops only the bad string, which is quieter — quiet enough to hide the typo
/// forever if this did not say so, since the file keeps it.
pub(super) fn notify_invalid_keybindings(
    input_state: &mut InputState,
    tokio_handle: &tokio::runtime::Handle,
    invalid: &[InvalidKeybinding],
) {
    if invalid.is_empty() {
        return;
    }

    input_state.push_toast(
        ToastPriority::Action,
        "keybindings.invalid",
        Toast::warning(invalid_keybinding_toast(invalid))
            .action("Shortcuts", Action::OpenConfiguratorKeybindings)
            .duration_ms(KEYBINDING_CONFLICT_TOAST_MS),
    );
    notification::send_notification_with_timeout_async(
        tokio_handle,
        "Invalid Shortcuts".to_string(),
        invalid_keybinding_notification_body(invalid, &config_path_display()),
        Some("dialog-warning".to_string()),
        KEYBINDING_CONFLICT_NOTIFICATION_TIMEOUT_MS,
    );
}

/// Tells the user which newly shipped default shortcut is not in effect.
///
/// This one is about us, not about their file: an action they never configured
/// was given a default this build introduced, and their own configuration
/// already spends that key. Nothing they authored changed, so it is
/// informational — no warning styling and no offer to fix anything — but it
/// still has to be said, or a shortcut the release notes promise would simply
/// appear not to work.
///
/// It carries the same Shortcuts chip as its neighbours — that is where the
/// user would add the shortcut — so it shares their priority class, but it is
/// pushed last and styled as information rather than as a warning.
pub(super) fn notify_skipped_default_shortcuts(
    input_state: &mut InputState,
    tokio_handle: &tokio::runtime::Handle,
    skipped: &[DefaultShortcutSkipped],
) {
    if skipped.is_empty() {
        return;
    }

    input_state.push_toast(
        ToastPriority::Action,
        "keybindings.skipped-default",
        Toast::info(skipped_default_toast(skipped))
            .action("Shortcuts", Action::OpenConfiguratorKeybindings)
            .duration_ms(KEYBINDING_CONFLICT_TOAST_MS),
    );
    notification::send_notification_with_timeout_async(
        tokio_handle,
        "New Default Shortcuts".to_string(),
        skipped_default_notification_body(skipped, &config_path_display()),
        Some("dialog-information".to_string()),
        KEYBINDING_CONFLICT_NOTIFICATION_TIMEOUT_MS,
    );
}

fn keybinding_conflict_toast(conflicts: &[KeybindingConflictResolution]) -> String {
    keybinding_toast(
        "Shortcut conflict",
        "shortcut conflicts",
        &summaries(conflicts, KeybindingConflictResolution::summary),
    )
}

fn invalid_keybinding_toast(invalid: &[InvalidKeybinding]) -> String {
    keybinding_toast(
        "Invalid shortcut",
        "invalid shortcuts",
        &summaries(invalid, InvalidKeybinding::summary),
    )
}

fn keybinding_conflict_notification_body(
    conflicts: &[KeybindingConflictResolution],
    config_path: &str,
) -> String {
    keybinding_notification_body(
        &summaries(conflicts, ToString::to_string),
        &format!(
            "Nothing was changed in {config_path}; edit it to choose which action keeps each shortcut."
        ),
    )
}

fn invalid_keybinding_notification_body(
    invalid: &[InvalidKeybinding],
    config_path: &str,
) -> String {
    keybinding_notification_body(
        &summaries(invalid, ToString::to_string),
        &format!("Nothing was changed in {config_path}; fix the spelling there to bind it again."),
    )
}

fn skipped_default_toast(skipped: &[DefaultShortcutSkipped]) -> String {
    keybinding_toast(
        "New default shortcut inactive",
        "new default shortcuts are inactive",
        &summaries(skipped, DefaultShortcutSkipped::summary),
    )
}

fn skipped_default_notification_body(
    skipped: &[DefaultShortcutSkipped],
    config_path: &str,
) -> String {
    keybinding_notification_body(
        &summaries(skipped, ToString::to_string),
        &format!(
            "Nothing was changed in {config_path}; add the shortcut there if you want it instead."
        ),
    )
}

fn summaries<T>(entries: &[T], render: impl Fn(&T) -> String) -> Vec<String> {
    entries.iter().map(render).collect()
}

/// A toast has room for one problem, so the rest are only counted.
fn keybinding_toast(singular: &str, plural: &str, entries: &[String]) -> String {
    let Some(first) = entries.first() else {
        return String::new();
    };
    if entries.len() == 1 {
        return format!("{singular}: {first}");
    }
    format!(
        "{} {plural}: {first} (and {} more)",
        entries.len(),
        entries.len() - 1
    )
}

fn keybinding_notification_body(entries: &[String], closing: &str) -> String {
    let mut body = entries
        .iter()
        .take(KEYBINDING_CONFLICT_NOTIFICATION_LIMIT)
        .map(|entry| format!("• {entry}."))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(remaining) = entries
        .len()
        .checked_sub(KEYBINDING_CONFLICT_NOTIFICATION_LIMIT)
        .filter(|remaining| *remaining > 0)
    {
        body.push_str(&format!("\n• and {remaining} more."));
    }
    body.push('\n');
    body.push_str(closing);
    body
}

fn config_path_display() -> String {
    Config::get_config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.config/wayscriber/config.toml".to_string())
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

    /// An old revision is read and reported on, never repaired: the file that
    /// comes out of startup is byte-identical to the one that went in.
    #[test]
    fn load_leaves_a_legacy_config_exactly_as_written() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            let original =
                "[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\nundo = [\"Ctrl+Alt+U\"]\n";
            fs::write(&path, original).expect("write legacy config");

            let loaded = load(ExitAfterCaptureMode::Auto);

            assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
            assert_eq!(
                loaded.config.config_revision, 0,
                "startup does not stamp a revision"
            );
            assert_eq!(fs::read_to_string(&path).expect("read config"), original);
        });
    }

    /// The reporter's file (#293): the authored `toggle_toolbar` meets the
    /// `cycle_toolbar_display` default the file never mentions. The default is
    /// the one that stands down, and startup says so — informationally, since
    /// nothing the user wrote lost anything.
    #[test]
    fn load_reports_a_skipped_default_instead_of_resetting_the_section() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            let original = format!(
                "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\nexit = [\"Escape\", \"Ctrl+Q\", \"Q\"]\n",
                crate::config::CURRENT_CONFIG_REVISION
            );
            fs::write(&path, &original).expect("write colliding config");

            let loaded = load(ExitAfterCaptureMode::Auto);

            assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
            assert_eq!(
                loaded.config.keybindings.core.exit,
                ["Escape", "Ctrl+Q", "Q"]
            );
            assert!(
                loaded
                    .config
                    .keybindings
                    .ui
                    .cycle_toolbar_display
                    .is_empty()
            );
            assert!(loaded.keybindings.keybinding_conflicts.is_empty());
            assert_eq!(loaded.keybindings.skipped_default_shortcuts.len(), 1);
            assert_eq!(
                loaded.keybindings.skipped_default_shortcuts[0].binding(),
                "F2"
            );

            let toast = skipped_default_toast(&loaded.keybindings.skipped_default_shortcuts);
            assert!(toast.contains("F2"), "unexpected toast: {toast}");
            assert!(
                toast.contains("Toggle Toolbar") && toast.contains("Cycle Toolbar Display"),
                "the toast must name both actions: {toast}"
            );

            let body = skipped_default_notification_body(
                &loaded.keybindings.skipped_default_shortcuts,
                "/tmp/config.toml",
            );
            assert!(body.contains("F2"), "unexpected body: {body}");
            assert!(
                body.contains("Nothing was changed"),
                "unexpected body: {body}"
            );
            assert!(body.contains("/tmp/config.toml"), "unexpected body: {body}");
            assert_eq!(fs::read_to_string(&path).expect("read config"), original);
        });
    }

    /// Two authored actions claiming one key is still the user's conflict, and
    /// startup still has to hand it to them: the resolution is session-only and
    /// the file keeps the collision.
    #[test]
    fn load_reports_a_conflict_between_two_authored_shortcuts() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            let original = format!(
                "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\"]\ncycle_toolbar_display = [\"F2\"]\n",
                crate::config::CURRENT_CONFIG_REVISION
            );
            fs::write(&path, &original).expect("write colliding config");

            let loaded = load(ExitAfterCaptureMode::Auto);

            assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F2"]);
            assert!(
                loaded
                    .config
                    .keybindings
                    .ui
                    .cycle_toolbar_display
                    .is_empty()
            );
            assert_eq!(loaded.keybindings.keybinding_conflicts.len(), 1);
            assert_eq!(loaded.keybindings.keybinding_conflicts[0].key(), "F2");
            assert!(loaded.keybindings.skipped_default_shortcuts.is_empty());

            let toast = keybinding_conflict_toast(&loaded.keybindings.keybinding_conflicts);
            assert!(
                toast.contains("Toggle Toolbar") && toast.contains("Cycle Toolbar Display"),
                "the toast must name both actions: {toast}"
            );

            let body = keybinding_conflict_notification_body(
                &loaded.keybindings.keybinding_conflicts,
                "/tmp/config.toml",
            );
            assert!(body.contains("F2"), "unexpected body: {body}");
            assert!(body.contains("/tmp/config.toml"), "unexpected body: {body}");
            assert_eq!(fs::read_to_string(&path).expect("read config"), original);
        });
    }

    /// A mistyped shortcut is dropped from the session keymap and left in the
    /// file, so startup is the only place the user learns the key they pressed
    /// will never fire. Everything else in the section keeps working, which is
    /// exactly what makes the typo easy to miss.
    #[test]
    fn load_reports_an_unparseable_shortcut_instead_of_defaulting_the_section() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            fs::write(
                &path,
                format!(
                    "config_revision = {}\n\n[keybindings]\nclear_canvas = [\"Ctrl+Shift\"]\nundo = [\"Ctrl+Alt+U\"]\n",
                    crate::config::CURRENT_CONFIG_REVISION
                ),
            )
            .expect("write config with a typo");

            let loaded = load(ExitAfterCaptureMode::Auto);

            assert!(loaded.config.keybindings.core.clear_canvas.is_empty());
            assert_eq!(loaded.config.keybindings.core.undo, ["Ctrl+Alt+U"]);
            assert_eq!(loaded.keybindings.invalid_keybindings.len(), 1);
            assert_eq!(
                loaded.keybindings.invalid_keybindings[0].binding(),
                "Ctrl+Shift"
            );
            assert!(loaded.keybindings.keybinding_conflicts.is_empty());

            let toast = invalid_keybinding_toast(&loaded.keybindings.invalid_keybindings);
            assert!(toast.contains("Ctrl+Shift"), "unexpected toast: {toast}");
            assert!(
                toast.contains("Clear Canvas"),
                "the toast must name the action: {toast}"
            );

            let body = invalid_keybinding_notification_body(
                &loaded.keybindings.invalid_keybindings,
                "/tmp/config.toml",
            );
            assert!(body.contains("Ctrl+Shift"), "unexpected body: {body}");
            assert!(
                body.contains("ignored for this session"),
                "unexpected body: {body}"
            );
            assert!(body.contains("/tmp/config.toml"), "unexpected body: {body}");
        });
    }

    #[test]
    fn keybinding_conflict_notification_body_truncates_long_lists() {
        let mut config = Config::default();
        config.keybindings.core.undo = vec!["Escape".to_string()];
        config.keybindings.core.redo = vec!["Ctrl+Q".to_string()];
        let conflicts = config.validate_and_clamp().keybinding_conflicts;
        assert_eq!(conflicts.len(), 2, "fixture should collide twice");

        let toast = keybinding_conflict_toast(&conflicts);
        assert!(
            toast.starts_with("2 shortcut conflicts"),
            "unexpected toast: {toast}"
        );
        assert!(toast.contains("and 1 more"), "unexpected toast: {toast}");

        let many = std::iter::repeat_n(conflicts[0].clone(), 7).collect::<Vec<_>>();
        let body = keybinding_conflict_notification_body(&many, "/tmp/config.toml");
        assert_eq!(
            body.matches('•').count(),
            KEYBINDING_CONFLICT_NOTIFICATION_LIMIT + 1
        );
        assert!(body.contains("and 2 more."), "unexpected body: {body}");
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
            assert!(
                matches!(loaded.load_failure, Some(ConfigLoadFailure::File { .. })),
                "a total fallback must be reported, not only logged"
            );
            assert!(
                loaded
                    .load_failure
                    .as_ref()
                    .is_some_and(|failure| failure.section_failed("session")),
                "a whole-file fallback must fail closed for session mutations"
            );
        });
    }

    /// One bad value costs its own section for the session; everything else
    /// the user authored stays in effect, and startup says which section is
    /// running on defaults.
    #[test]
    fn load_keeps_the_rest_of_the_file_when_one_section_is_invalid() {
        with_temp_config_home(|_| {
            let path = Config::get_config_path().expect("config path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create config dir");
            }
            let original = "[ui]\ntheme = \"drak\"\n\n[capture]\nexit_after_capture = true\n";
            fs::write(&path, original).expect("write config with a bad value");

            let loaded = load(ExitAfterCaptureMode::Auto);

            assert!(matches!(loaded.source, ConfigSource::Primary));
            assert!(
                loaded.config.capture.exit_after_capture,
                "sections that map cleanly must stay in effect"
            );
            let Some(ConfigLoadFailure::Sections(sections)) = &loaded.load_failure else {
                panic!("expected a per-section load failure report");
            };
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].section, "ui");
            let failure = loaded.load_failure.as_ref().expect("section failure");
            assert!(failure.section_failed("ui"));
            assert!(!failure.section_failed("session"));
            assert_eq!(
                fs::read_to_string(&path).expect("read config"),
                original,
                "loading repairs the session, never the file"
            );
        });
    }
}
