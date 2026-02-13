use iced::Command;

use crate::messages::Message;
use crate::models::{
    BoardBackgroundOption, BoardItemTextField, BoardItemToggleField, ColorPickerId,
};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_boards_add_item(&mut self) -> Command<Message> {
        self.status = StatusMessage::idle();
        let new_item = self.draft.boards.new_item();
        self.draft.boards.items.push(new_item);
        self.boards_collapsed.push(false);
        self.clear_board_color_pickers();
        self.draft.boards.ensure_default_exists();
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_remove_item(&mut self, index: usize) -> Command<Message> {
        self.status = StatusMessage::idle();
        if index < self.draft.boards.items.len() {
            self.draft.boards.items.remove(index);
            if index < self.boards_collapsed.len() {
                self.boards_collapsed.remove(index);
            }
            self.clear_board_color_pickers();
            self.draft.boards.ensure_default_exists();
            self.refresh_dirty_flag();
        }
        Command::none()
    }

    pub(super) fn handle_boards_move_item(&mut self, index: usize, up: bool) -> Command<Message> {
        self.status = StatusMessage::idle();
        let len = self.draft.boards.items.len();
        if len <= 1 {
            return Command::none();
        }
        let target = if up {
            if index == 0 {
                return Command::none();
            }
            index - 1
        } else {
            if index + 1 >= len {
                return Command::none();
            }
            index + 1
        };
        self.draft.boards.items.swap(index, target);
        if index < self.boards_collapsed.len() && target < self.boards_collapsed.len() {
            self.boards_collapsed.swap(index, target);
        }
        self.clear_board_color_pickers();
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_duplicate_item(&mut self, index: usize) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get(index).cloned() {
            let mut duplicate = item;
            duplicate.id = self.draft.boards.next_board_id();
            if !duplicate.name.trim().is_empty() {
                duplicate.name = format!("{} Copy", duplicate.name.trim());
            }
            let insert_index = index + 1;
            self.draft.boards.items.insert(insert_index, duplicate);
            self.boards_collapsed.insert(insert_index, false);
            self.clear_board_color_pickers();
            self.draft.boards.ensure_default_exists();
            self.refresh_dirty_flag();
        }
        Command::none()
    }

    pub(super) fn handle_boards_collapse_toggled(&mut self, index: usize) -> Command<Message> {
        if let Some(value) = self.boards_collapsed.get_mut(index) {
            *value = !*value;
        }
        Command::none()
    }

    pub(super) fn handle_boards_default_changed(&mut self, value: String) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.boards.default_board = value;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_item_text_changed(
        &mut self,
        index: usize,
        field: BoardItemTextField,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        let old_effective_id = self.draft.boards.effective_id_for_index(index);
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            match field {
                BoardItemTextField::Id => {
                    let trimmed = value.trim();
                    let new_effective_id = if trimmed.is_empty() {
                        format!("board-{}", index + 1)
                    } else {
                        trimmed.to_string()
                    };
                    item.id = value;
                    if let Some(old_effective_id) = old_effective_id
                        && self.draft.boards.default_board == old_effective_id
                    {
                        self.draft.boards.default_board = new_effective_id;
                    }
                }
                BoardItemTextField::Name => {
                    item.name = value;
                }
            }
        }
        self.draft.boards.ensure_default_exists();
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_background_kind_changed(
        &mut self,
        index: usize,
        value: BoardBackgroundOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.background_kind = value;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_background_color_changed(
        &mut self,
        index: usize,
        component: usize,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.background_color.set_component(component, value);
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::BoardBackground(index));
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_default_pen_enabled_changed(
        &mut self,
        index: usize,
        value: bool,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.default_pen_color.enabled = value;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_default_pen_color_changed(
        &mut self,
        index: usize,
        component: usize,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.default_pen_color.color.set_component(component, value);
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::BoardPen(index));
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_boards_item_toggle_changed(
        &mut self,
        index: usize,
        field: BoardItemToggleField,
        value: bool,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            match field {
                BoardItemToggleField::AutoAdjustPen => item.auto_adjust_pen = value,
                BoardItemToggleField::Persist => item.persist = value,
                BoardItemToggleField::Pinned => item.pinned = value,
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    fn clear_board_color_pickers(&mut self) {
        if matches!(
            self.color_picker_open,
            Some(ColorPickerId::BoardBackground(_) | ColorPickerId::BoardPen(_))
        ) {
            self.color_picker_open = None;
        }
        self.color_picker_hex.retain(|id, _| {
            !matches!(
                id,
                ColorPickerId::BoardBackground(_) | ColorPickerId::BoardPen(_)
            )
        });
        self.color_picker_advanced.retain(|id| {
            !matches!(
                id,
                ColorPickerId::BoardBackground(_) | ColorPickerId::BoardPen(_)
            )
        });
        self.sync_board_color_picker_hex();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ColorPickerId;

    #[test]
    fn add_item_updates_boards_and_collapsed_state() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let before = app.draft.boards.items.len();

        let _ = app.handle_boards_add_item();

        assert_eq!(app.draft.boards.items.len(), before + 1);
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
        assert!(app.is_dirty);
    }

    #[test]
    fn duplicate_item_inserts_copy_with_new_id() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let before = app.draft.boards.items.len();
        let original_id = app
            .draft
            .boards
            .items
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_default();

        let _ = app.handle_boards_duplicate_item(0);

        assert_eq!(app.draft.boards.items.len(), before + 1);
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
        assert_ne!(app.draft.boards.items[1].id, original_id);
    }

    #[test]
    fn remove_item_clears_board_color_picker_when_targeting_board_entry() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let _ = app.handle_boards_add_item();
        app.color_picker_open = Some(ColorPickerId::BoardBackground(0));

        let _ = app.handle_boards_remove_item(0);

        assert!(app.color_picker_open.is_none());
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
    }
}
