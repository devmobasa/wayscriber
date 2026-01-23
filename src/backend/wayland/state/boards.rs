use crate::config::BoardsConfig;

use super::WaylandState;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_board_config_update(&mut self, boards: BoardsConfig) {
        self.config.boards = Some(boards);
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save board config: {}", err);
        }
    }
}
