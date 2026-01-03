use iced::Command;

use crate::messages::Message;
use crate::models::{
    BoardModeOption, ColorMode, EraserModeOption, FontStyleOption, FontWeightOption,
    KeybindingField, NamedColorOption, OverrideOption, PresenterToolBehaviorOption, QuadField,
    SessionCompressionOption, SessionStorageModeOption, StatusPositionOption, TextField,
    ToggleField, ToolbarLayoutModeOption, ToolbarOverrideField, TripletField,
};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_toggle_changed(
        &mut self,
        field: ToggleField,
        value: bool,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_toggle(field, value);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_text_changed(
        &mut self,
        field: TextField,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_text(field, value);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_triplet_changed(
        &mut self,
        field: TripletField,
        index: usize,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_triplet(field, index, value);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_quad_changed(
        &mut self,
        field: QuadField,
        index: usize,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_quad(field, index, value);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_color_mode_changed(&mut self, mode: ColorMode) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.mode = mode;
        if matches!(mode, ColorMode::Named) {
            if self.draft.drawing_color.name.trim().is_empty() {
                self.draft.drawing_color.selected_named = NamedColorOption::Red;
                self.draft.drawing_color.name = self
                    .draft
                    .drawing_color
                    .selected_named
                    .as_value()
                    .to_string();
            } else {
                self.draft.drawing_color.update_named_from_current();
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_named_color_selected(
        &mut self,
        option: NamedColorOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.selected_named = option;
        if option != NamedColorOption::Custom {
            self.draft.drawing_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_eraser_mode_changed(
        &mut self,
        option: EraserModeOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_default_eraser_mode = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_status_position_changed(
        &mut self,
        option: StatusPositionOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.ui_status_position = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_toolbar_layout_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.apply_toolbar_layout_mode(option);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_toolbar_override_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Command<Message> {
        self.override_mode = option;
        Command::none()
    }

    pub(super) fn handle_toolbar_override_changed(
        &mut self,
        field: ToolbarOverrideField,
        option: OverrideOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft
            .set_toolbar_override(self.override_mode, field, option);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_board_mode_changed(
        &mut self,
        option: BoardModeOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.board_default_mode = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_session_storage_mode_changed(
        &mut self,
        option: SessionStorageModeOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.session_storage_mode = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_session_compression_changed(
        &mut self,
        option: SessionCompressionOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.session_compression = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_presenter_tool_behavior_changed(
        &mut self,
        option: PresenterToolBehaviorOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.presenter_tool_behavior = option;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_buffer_count_changed(&mut self, count: u32) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.performance_buffer_count = count;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_keybinding_changed(
        &mut self,
        field: KeybindingField,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.keybindings.set(field, value);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_font_style_option_selected(
        &mut self,
        option: FontStyleOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_style_option = option;
        if option != FontStyleOption::Custom {
            self.draft.drawing_font_style = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_font_weight_option_selected(
        &mut self,
        option: FontWeightOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_weight_option = option;
        if option != FontWeightOption::Custom {
            self.draft.drawing_font_weight = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Command::none()
    }
}
