use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_boards(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        let boards = self.boards.to_config(errors);
        config.boards = Some(boards);
    }
}
