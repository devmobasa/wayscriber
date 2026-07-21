use super::super::CURRENT_CONFIG_REVISION;
use super::super::keybindings::KeybindingsConfig;
use super::Config;

const LEGACY_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K"];
const CURRENT_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K", "Ctrl+Shift+P"];
const LEGACY_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Shift+P"];
const CURRENT_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Alt+F"];
const LEGACY_TOGGLE_TOOLBAR_DEFAULT: &[&str] = &["F2", "F9"];
const CURRENT_TOGGLE_TOOLBAR_DEFAULT: &[&str] = &["F9"];
const CURRENT_CYCLE_TOOLBAR_DISPLAY_DEFAULT: &[&str] = &["F2"];

fn bindings_equal(bindings: &[String], expected: &[&str]) -> bool {
    bindings
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn bindings_from(expected: &[&str]) -> Vec<String> {
    expected
        .iter()
        .map(|binding| (*binding).to_string())
        .collect()
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
        // Each step is gated on the revision that introduced it, so a config
        // saved at a later revision never re-runs an earlier heuristic (a
        // deliberately restored legacy value must survive future upgrades).
        if self.config_revision < 1 {
            self.migrate_command_palette_and_capture_defaults();
        }
        if self.config_revision < 2 {
            self.migrate_toggle_toolbar_f2_split();
        }
        self.config_revision = CURRENT_CONFIG_REVISION;
    }

    /// Revision 1: `Ctrl+K`-only command palette and `Ctrl+Shift+P`
    /// full-screen capture defaults moved to `Ctrl+K`/`Ctrl+Shift+P` and
    /// `Ctrl+Alt+F`.
    fn migrate_command_palette_and_capture_defaults(&mut self) {
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
            self.keybindings.ui.toggle_command_palette =
                bindings_from(CURRENT_COMMAND_PALETTE_DEFAULT);
            self.keybindings.capture.capture_full_screen =
                bindings_from(CURRENT_FULL_SCREEN_CAPTURE_DEFAULT);
            log::info!("Migrated legacy command-palette and full-screen capture default shortcuts");
        }
    }

    /// Revision 2: `F2` moved from the `toggle_toolbar` default pair to the
    /// new `cycle_toolbar_display` action. Without this step a config that
    /// explicitly lists the old `["F2", "F9"]` default would collide with
    /// the serde-defaulted `cycle_toolbar_display = ["F2"]`, and keybinding
    /// validation would then wipe every custom binding back to defaults.
    fn migrate_toggle_toolbar_f2_split(&mut self) {
        // `cycle_toolbar_display` did not exist before revision 2, so a
        // pre-revision file normally carries the serde default (`["F2"]`).
        // Any other value means the user already adopted the new field
        // deliberately — leave both sides untouched.
        if !bindings_equal(
            &self.keybindings.ui.cycle_toolbar_display,
            CURRENT_CYCLE_TOOLBAR_DISPLAY_DEFAULT,
        ) {
            return;
        }
        if bindings_equal(
            &self.keybindings.ui.toggle_toolbar,
            LEGACY_TOGGLE_TOOLBAR_DEFAULT,
        ) {
            // The shipped default pair: F2 moves to the cycle action and F9
            // keeps toggling visibility.
            self.keybindings.ui.toggle_toolbar = bindings_from(CURRENT_TOGGLE_TOOLBAR_DEFAULT);
            log::info!(
                "Migrated legacy toggle_toolbar default pair; F2 now cycles the toolbar display"
            );
        } else if self
            .keybindings
            .ui
            .toggle_toolbar
            .iter()
            .any(|binding| binding.trim().eq_ignore_ascii_case("F2"))
        {
            // A deliberate custom set that includes F2: the user's F2 keeps
            // its old toggle meaning and the cycle action starts unbound.
            self.keybindings.ui.cycle_toolbar_display = Vec::new();
            log::info!(
                "Preserved custom F2 toggle_toolbar binding; cycle_toolbar_display left unbound"
            );
        }
    }
}
