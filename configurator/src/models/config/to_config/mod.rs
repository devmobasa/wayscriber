mod boards;
mod capture;
mod drawing;
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

        self.apply_drawing(&mut config, &mut errors);
        self.apply_history(&mut config, &mut errors);
        self.apply_performance(&mut config, &mut errors);
        self.apply_ui(&mut config, &mut errors);
        self.apply_presenter_mode(&mut config);
        self.apply_boards(&mut config, &mut errors);
        self.apply_capture(&mut config, &mut errors);
        self.apply_session(&mut config, &mut errors);
        self.apply_tablet(&mut config, &mut errors);
        self.apply_presets(&mut config, &mut errors);
        self.apply_keybindings(&mut config, &mut errors);

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }
}
