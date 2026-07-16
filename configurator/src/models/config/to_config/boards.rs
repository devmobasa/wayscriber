use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_boards(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        if config.boards.is_none()
            && self.boards == super::super::boards::BoardsDraft::from_config(config)
        {
            return;
        }
        let boards = self.boards.to_config(errors);
        config.boards = Some(boards);
    }
}
