use super::super::CURRENT_CONFIG_REVISION;
use super::super::keybindings::KeybindingsConfig;
use super::Config;

const LEGACY_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K"];
const CURRENT_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K", "Ctrl+Shift+P"];
const LEGACY_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Shift+P"];
const CURRENT_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Alt+F"];

fn bindings_equal(bindings: &[String], expected: &[&str]) -> bool {
    bindings
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

impl Config {
    pub(super) fn validate_keybindings(&mut self) {
        self.apply_keybinding_migrations();
        // Validate keybindings (try to build action map to catch parse errors)
        if let Err(e) = self.keybindings.build_action_map() {
            log::warn!("Invalid keybinding configuration: {}. Using defaults.", e);
            self.keybindings = KeybindingsConfig::default();
        }
    }

    pub(crate) fn apply_keybinding_migrations(&mut self) {
        if self.config_revision >= CURRENT_CONFIG_REVISION {
            return;
        }
        let command_is_legacy = bindings_equal(
            &self.keybindings.ui.toggle_command_palette,
            LEGACY_COMMAND_PALETTE_DEFAULT,
        );
        let command_is_current = bindings_equal(
            &self.keybindings.ui.toggle_command_palette,
            CURRENT_COMMAND_PALETTE_DEFAULT,
        );
        let capture_is_legacy = bindings_equal(
            &self.keybindings.capture.capture_full_screen,
            LEGACY_FULL_SCREEN_CAPTURE_DEFAULT,
        );
        let capture_is_current = bindings_equal(
            &self.keybindings.capture.capture_full_screen,
            CURRENT_FULL_SCREEN_CAPTURE_DEFAULT,
        );

        // A missing field is filled by serde with its current default, so
        // accept legacy/current combinations as long as neither side is
        // customized. This keeps minimal legacy configs valid too.
        if (command_is_legacy || command_is_current)
            && (capture_is_legacy || capture_is_current)
            && (command_is_legacy || capture_is_legacy)
        {
            self.keybindings.ui.toggle_command_palette = CURRENT_COMMAND_PALETTE_DEFAULT
                .iter()
                .map(|binding| (*binding).to_string())
                .collect();
            self.keybindings.capture.capture_full_screen = CURRENT_FULL_SCREEN_CAPTURE_DEFAULT
                .iter()
                .map(|binding| (*binding).to_string())
                .collect();
            log::info!("Migrated legacy command-palette and full-screen capture default shortcuts");
        }
        self.config_revision = CURRENT_CONFIG_REVISION;
    }
}
