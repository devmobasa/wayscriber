mod item;

use crate::app::view::theme;
use iced::widget::{button, column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_usize_min,
};

impl ConfiguratorApp {
    pub(super) fn boards_tab(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let defaults = &self.defaults.boards;
        let boards = &self.draft.boards;
        let show_all = search.is_none_or(TabSearchSummary::show_all);
        let show_general =
            search.is_none_or(|search| search.area_matches(SearchArea::BoardsGeneral));

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

        let mut column = column![text("Boards").size(20)].spacing(12);

        if show_general || show_all {
            column = column
                .push(max_count)
                .push(auto_create)
                .push(show_badge)
                .push(persist_customizations)
                .push(default_board_control)
                .push(row![add_button].spacing(8));
        }

        if (show_general || show_all) && self.base_config.boards.is_none() {
            column = column.push(
                text("Legacy [board] settings detected. Saving will write [boards].")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
        }

        let indices: Vec<usize> = if show_all {
            (0..boards.items.len()).collect()
        } else {
            search
                .map(TabSearchSummary::board_indices)
                .unwrap_or_default()
                .to_vec()
        };

        for index in indices {
            column = column.push(self.board_item_section_for_search(index, !show_all));
        }

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }
}
