use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_board(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.board.enabled = self.board_enabled;
        config.board.default_mode = self.board_default_mode.as_str().to_string();
        match self
            .board_whiteboard_color
            .to_array("board.whiteboard_color")
        {
            Ok(values) => config.board.whiteboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_color
            .to_array("board.blackboard_color")
        {
            Ok(values) => config.board.blackboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_whiteboard_pen
            .to_array("board.whiteboard_pen_color")
        {
            Ok(values) => config.board.whiteboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_pen
            .to_array("board.blackboard_pen_color")
        {
            Ok(values) => config.board.blackboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        config.board.auto_adjust_pen = self.board_auto_adjust_pen;
    }
}
