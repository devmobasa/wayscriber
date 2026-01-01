use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_presets(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.presets = self.presets.to_config(errors);
    }
}
