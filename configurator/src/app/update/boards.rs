use crate::models::{
    BoardBackgroundOption, BoardItemTextField, BoardItemToggleField, ColorPickerId,
};

use super::super::effects::Effect;
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_boards_add_item(&mut self) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let new_item = self.draft.boards.new_item();
        self.draft.boards.items.push(new_item);
        self.boards_collapsed.push(false);
        self.clear_board_color_pickers();
        self.draft.boards.ensure_default_exists();
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_remove_item(&mut self, index: usize) -> Vec<Effect> {
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
        Vec::new()
    }

    pub(super) fn handle_boards_move_item(&mut self, index: usize, up: bool) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let len = self.draft.boards.items.len();
        if len <= 1 {
            return Vec::new();
        }
        let target = if up {
            if index == 0 {
                return Vec::new();
            }
            index - 1
        } else {
            if index + 1 >= len {
                return Vec::new();
            }
            index + 1
        };
        self.draft.boards.items.swap(index, target);
        if index < self.boards_collapsed.len() && target < self.boards_collapsed.len() {
            self.boards_collapsed.swap(index, target);
        }
        self.clear_board_color_pickers();
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_duplicate_item(&mut self, index: usize) -> Vec<Effect> {
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
        Vec::new()
    }

    pub(super) fn handle_boards_collapse_toggled(&mut self, index: usize) -> Vec<Effect> {
        if let Some(value) = self.boards_collapsed.get_mut(index) {
            *value = !*value;
        }
        Vec::new()
    }

    pub(super) fn handle_boards_default_changed(&mut self, value: String) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.boards.default_board = value;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_item_text_changed(
        &mut self,
        index: usize,
        field: BoardItemTextField,
        value: String,
    ) -> Vec<Effect> {
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
        Vec::new()
    }

    pub(super) fn handle_boards_background_kind_changed(
        &mut self,
        index: usize,
        value: BoardBackgroundOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let changed = if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.background_kind = value;
            true
        } else {
            false
        };
        if changed && value != BoardBackgroundOption::Color {
            // The picker is no longer reachable. Drop any half-typed buffer
            // and restore its canonical draft value so it cannot block Save.
            self.sync_color_picker_hex_for_id(ColorPickerId::BoardBackground(index));
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_background_color_changed(
        &mut self,
        index: usize,
        component: usize,
        value: String,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.background_color.set_component(component, value);
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::BoardBackground(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_default_pen_enabled_changed(
        &mut self,
        index: usize,
        value: bool,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let changed = if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.default_pen_color.enabled = value;
            true
        } else {
            false
        };
        if changed && !value {
            // Disabling the override hides this required-looking editor, so
            // abandon any incomplete text with the control that held it.
            self.sync_color_picker_hex_for_id(ColorPickerId::BoardPen(index));
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_default_pen_color_changed(
        &mut self,
        index: usize,
        component: usize,
        value: String,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            item.default_pen_color.color.set_component(component, value);
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::BoardPen(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_boards_item_toggle_changed(
        &mut self,
        index: usize,
        field: BoardItemToggleField,
        value: bool,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if let Some(item) = self.draft.boards.items.get_mut(index) {
            match field {
                BoardItemToggleField::AutoAdjustPen => item.auto_adjust_pen = value,
                BoardItemToggleField::Persist => item.persist = value,
                BoardItemToggleField::Pinned => item.pinned = value,
            }
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    fn clear_board_color_pickers(&mut self) {
        self.color_picker_hex.retain(|id, _| {
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

    #[test]
    fn add_item_updates_boards_and_collapsed_state() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let before = app.draft.boards.items.len();

        let _ = app.handle_boards_add_item();

        assert_eq!(app.draft.boards.items.len(), before + 1);
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
        assert!(app.is_dirty);
    }

    #[test]
    fn duplicate_item_inserts_copy_with_new_id() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
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
    fn remove_item_keeps_collapsed_state_in_step_with_the_board_list() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_boards_add_item();

        let _ = app.handle_boards_remove_item(0);

        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
    }

    #[test]
    fn hiding_a_board_background_color_releases_its_invalid_hex() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_boards_background_kind_changed(0, BoardBackgroundOption::Color);
        let _ = app.handle_color_picker_hex_changed(
            ColorPickerId::BoardBackground(0),
            "#12zz".to_string(),
        );
        assert_eq!(app.invalid_color_hex_count(), 1);

        let _ = app.handle_boards_background_kind_changed(0, BoardBackgroundOption::Transparent);

        assert_eq!(app.invalid_color_hex_count(), 0);
    }

    #[test]
    fn disabling_a_board_pen_override_releases_its_invalid_hex() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_boards_default_pen_enabled_changed(0, true);
        let _ =
            app.handle_color_picker_hex_changed(ColorPickerId::BoardPen(0), "#12zz".to_string());
        assert_eq!(app.invalid_color_hex_count(), 1);

        let _ = app.handle_boards_default_pen_enabled_changed(0, false);

        assert_eq!(app.invalid_color_hex_count(), 0);
    }
}
