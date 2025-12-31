use super::super::keybindings::KeybindingsConfig;
use super::Config;

impl Config {
    pub(super) fn validate_keybindings(&mut self) {
        // Validate keybindings (try to build action map to catch parse errors)
        if let Err(e) = self.keybindings.build_action_map() {
            log::warn!("Invalid keybinding configuration: {}. Using defaults.", e);
            self.keybindings = KeybindingsConfig::default();
        }
    }
}
