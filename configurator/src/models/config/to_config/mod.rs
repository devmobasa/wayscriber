mod boards;
mod capture;
mod drawing;
mod export;
mod history;
mod keybindings;
mod performance;
mod presenter_mode;
mod presets;
mod session;
mod tablet;
mod ui;

use super::super::error::FormError;
use super::draft::ConfigDraft;
use wayscriber::config::Config;

impl ConfigDraft {
    pub fn to_config(&self, base: &Config) -> Result<Config, Vec<FormError>> {
        let mut errors = Vec::new();
        let mut config = base.clone();

        // Only an applied migration proposes a revision; otherwise the base
        // document's own value is what a save writes back, so an unrelated
        // edit to an old file leaves its revision exactly where it was.
        if let Some(revision) = self.config_revision {
            config.config_revision = revision;
        }
        self.apply_drawing(&mut config, &mut errors);
        self.apply_history(&mut config, &mut errors);
        self.apply_performance(&mut config, &mut errors);
        self.apply_ui(&mut config, &mut errors);
        self.apply_presenter_mode(&mut config);
        self.apply_boards(&mut config, &mut errors);
        self.render_profiles
            .apply_to_config(&mut config, &mut errors);
        self.apply_capture(&mut config, &mut errors);
        self.apply_export(&mut config, &mut errors);
        self.apply_session(&mut config, &mut errors);
        self.apply_tablet(&mut config, &mut errors);
        self.apply_presets(&mut config, &mut errors);
        self.apply_keybindings(&mut config, &mut errors);
        // `apply_keybindings` rebuilt the whole section from the editor's text
        // fields, so the base document's record of which keys its file spells
        // out no longer describes these lists. Saying so is what keeps a
        // shortcut the user typed for an action their file omits from being
        // treated as a compiled-in default and dropped by validation.
        config.mark_keybindings_explicit();

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }
}
