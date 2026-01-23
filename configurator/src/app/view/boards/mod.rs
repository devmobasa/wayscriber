mod item;

use iced::theme;
use iced::widget::{button, column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_usize_min,
};

impl ConfiguratorApp {
    pub(super) fn boards_tab(&self) -> Element<'_, Message> {
        let defaults = &self.defaults.boards;
        let boards = &self.draft.boards;

        let max_count = labeled_input_with_feedback(
            "Max boards",
            &boards.max_count,
            &defaults.max_count,
            TextField::BoardsMaxCount,
            Some("Minimum: 1"),
            validate_usize_min(&boards.max_count, 1),
        );

        let auto_create = toggle_row(
            "Auto-create missing boards",
            boards.auto_create,
            defaults.auto_create,
            ToggleField::BoardsAutoCreate,
        );

        let show_badge = toggle_row(
            "Show board badge",
            boards.show_board_badge,
            defaults.show_board_badge,
            ToggleField::BoardsShowBadge,
        );

        let persist_customizations = toggle_row(
            "Persist runtime customizations",
            boards.persist_customizations,
            defaults.persist_customizations,
            ToggleField::BoardsPersistCustomizations,
        );

        let board_ids = boards.effective_ids();
        let selection = if board_ids.contains(&boards.default_board) {
            Some(boards.default_board.clone())
        } else {
            None
        };

        let default_board_control = if board_ids.is_empty() {
            labeled_control(
                "Default board",
                text("Add a board to choose a default").size(12).into(),
                defaults.default_board.clone(),
                boards.default_board != defaults.default_board,
            )
        } else {
            let picker =
                pick_list(board_ids, selection, Message::BoardsDefaultChanged).width(Length::Fill);
            labeled_control(
                "Default board",
                picker.into(),
                defaults.default_board.clone(),
                boards.default_board != defaults.default_board,
            )
        };

        let add_button = button("Add board").on_press(Message::BoardsAddItem);

        let mut column = column![
            text("Boards").size(20),
            max_count,
            auto_create,
            show_badge,
            persist_customizations,
            default_board_control,
            row![add_button].spacing(8),
        ]
        .spacing(12);

        if self.base_config.boards.is_none() {
            column = column.push(
                text("Legacy [board] settings detected. Saving will write [boards].")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
        }

        for index in 0..boards.items.len() {
            column = column.push(self.board_item_section(index));
        }

        scrollable(column).into()
    }
}
