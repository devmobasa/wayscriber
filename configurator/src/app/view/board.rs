use iced::widget::{column, pick_list, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{BoardModeOption, ToggleField, TripletField};

use super::super::state::ConfiguratorApp;
use super::widgets::{color_triplet_editor, labeled_control, toggle_row};

impl ConfiguratorApp {
    pub(super) fn board_tab(&self) -> Element<'_, Message> {
        let board_mode_pick = pick_list(
            BoardModeOption::list(),
            Some(self.draft.board_default_mode),
            Message::BoardModeChanged,
        );

        let column = column![
            text("Board Mode").size(20),
            toggle_row(
                "Enable board mode",
                self.draft.board_enabled,
                self.defaults.board_enabled,
                ToggleField::BoardEnabled,
            ),
            labeled_control(
                "Default mode",
                board_mode_pick.width(Length::Fill).into(),
                self.defaults.board_default_mode.label().to_string(),
                self.draft.board_default_mode != self.defaults.board_default_mode,
            ),
            color_triplet_editor(
                "Whiteboard color RGB (0-1)",
                &self.draft.board_whiteboard_color,
                &self.defaults.board_whiteboard_color,
                TripletField::BoardWhiteboard,
            ),
            color_triplet_editor(
                "Blackboard color RGB (0-1)",
                &self.draft.board_blackboard_color,
                &self.defaults.board_blackboard_color,
                TripletField::BoardBlackboard,
            ),
            color_triplet_editor(
                "Whiteboard pen RGB (0-1)",
                &self.draft.board_whiteboard_pen,
                &self.defaults.board_whiteboard_pen,
                TripletField::BoardWhiteboardPen,
            ),
            color_triplet_editor(
                "Blackboard pen RGB (0-1)",
                &self.draft.board_blackboard_pen,
                &self.defaults.board_blackboard_pen,
                TripletField::BoardBlackboardPen,
            ),
            toggle_row(
                "Auto-adjust pen color",
                self.draft.board_auto_adjust_pen,
                self.defaults.board_auto_adjust_pen,
                ToggleField::BoardAutoAdjust,
            )
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
