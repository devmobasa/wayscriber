use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_keybindings(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        match self.keybindings.to_config() {
            Ok(cfg) => config.keybindings = cfg,
            Err(errs) => errors.extend(errs),
        }
    }
}
