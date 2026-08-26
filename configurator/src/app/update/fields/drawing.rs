use crate::models::{
    ArrowStyleOption, ColorMode, ColorPickerId, DragColorOption, DragMouseButton, DragToolField,
    DragToolOption, EraserModeOption, NamedColorOption,
};

use super::super::super::effects::Effect;
use super::super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(in crate::app::update) fn handle_color_mode_changed(
        &mut self,
        mode: ColorMode,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if matches!(mode, ColorMode::Rgb) {
            self.draft.drawing_color.sync_rgb_from_preview();
        }
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
        self.sync_color_picker_hex_for_id(ColorPickerId::DrawingColor);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_named_color_selected(
        &mut self,
        option: NamedColorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.selected_named = option;
        if option != NamedColorOption::Custom {
            self.draft.drawing_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_quick_color_mode_changed(
        &mut self,
        index: usize,
        mode: ColorMode,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(entry) = self.draft.drawing_quick_colors.get_mut(index) else {
            return Vec::new();
        };
        let quick_color = &mut entry.color;
        if matches!(mode, ColorMode::Rgb) {
            quick_color.sync_rgb_from_preview();
        }
        quick_color.mode = mode;
        if matches!(mode, ColorMode::Named) {
            if quick_color.name.trim().is_empty() {
                quick_color.selected_named = NamedColorOption::Red;
                quick_color.name = quick_color.selected_named.as_value().to_string();
            } else {
                quick_color.update_named_from_current();
            }
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::QuickColor(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_quick_named_color_selected(
        &mut self,
        index: usize,
        option: NamedColorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(entry) = self.draft.drawing_quick_colors.get_mut(index) else {
            return Vec::new();
        };
        let quick_color = &mut entry.color;
        quick_color.selected_named = option;
        if option != NamedColorOption::Custom {
            quick_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_quick_color_added(&mut self) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let new_index = self.draft.drawing_quick_colors.entries.len();
        self.draft.drawing_quick_colors.add_entry();
        self.remap_quick_color_pickers(|index| (index < new_index).then_some(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_quick_color_removed(
        &mut self,
        index: usize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if self.draft.drawing_quick_colors.remove_entry(index) {
            self.remap_quick_color_pickers(|picker_index| match picker_index.cmp(&index) {
                std::cmp::Ordering::Less => Some(picker_index),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => picker_index.checked_sub(1),
            });
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    pub(in crate::app::update) fn handle_quick_color_moved(
        &mut self,
        index: usize,
        delta: isize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(target) = index.checked_add_signed(delta) else {
            return Vec::new();
        };
        if self.draft.drawing_quick_colors.move_entry(index, delta) {
            self.remap_quick_color_pickers(|picker_index| {
                if picker_index == target {
                    Some(index)
                } else if picker_index == index {
                    Some(target)
                } else {
                    Some(picker_index)
                }
            });
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    pub(in crate::app::update) fn handle_font_cycle_added(&mut self) -> Vec<Effect> {
        match self.draft.drawing_font_cycle.add() {
            Ok(()) => {
                self.status = StatusMessage::idle();
                self.refresh_dirty_flag();
            }
            Err(error) => self.status = StatusMessage::warning(error.to_string()),
        }
        Vec::new()
    }

    pub(in crate::app::update) fn handle_font_cycle_removed(
        &mut self,
        index: usize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if self.draft.drawing_font_cycle.remove(index) {
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    pub(in crate::app::update) fn handle_font_cycle_moved(
        &mut self,
        index: usize,
        delta: isize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if self.draft.drawing_font_cycle.move_entry(index, delta) {
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    pub(in crate::app::update) fn handle_font_cycle_changed(
        &mut self,
        index: usize,
        family: String,
    ) -> Vec<Effect> {
        match self.draft.drawing_font_cycle.set(index, family) {
            Ok(changed) => {
                self.status = StatusMessage::idle();
                if changed {
                    self.refresh_dirty_flag();
                }
            }
            Err(error) => self.status = StatusMessage::warning(error.to_string()),
        }
        Vec::new()
    }

    /// Carries each surviving quick-color edit buffer to its row's new index.
    ///
    /// The buffer can be half-typed and therefore absent from the draft. Add,
    /// move, and removal must preserve it with the row that survives, while a
    /// deleted row's buffer must disappear so it cannot hold Save hostage.
    fn remap_quick_color_pickers(&mut self, remap: impl Fn(usize) -> Option<usize>) {
        let count = self.draft.drawing_quick_colors.entries.len();
        let mut preserved = Vec::new();
        self.color_picker_hex.retain(|id, text| match id {
            ColorPickerId::QuickColor(index) => {
                if let Some(next) = remap(*index).filter(|next| *next < count) {
                    preserved.push((next, text.clone()));
                }
                false
            }
            _ => true,
        });
        for (index, text) in preserved {
            self.color_picker_hex
                .insert(ColorPickerId::QuickColor(index), text);
        }
        for index in 0..count {
            let id = ColorPickerId::QuickColor(index);
            if !self.color_picker_hex.contains_key(&id) {
                self.sync_color_picker_hex_for_id(id);
            }
        }
    }

    pub(in crate::app::update) fn handle_eraser_mode_changed(
        &mut self,
        option: EraserModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_default_eraser_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    /// `[arrow] style`: which shape the next arrow is drawn in.
    ///
    /// Seeds new arrows only. Nothing already drawn is restyled, because every
    /// arrow stores its own style.
    pub(in crate::app::update) fn handle_arrow_style_changed(
        &mut self,
        option: ArrowStyleOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.arrow_style = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_drawing_mouse_drag_tool_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragToolOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_tool(button, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_drawing_mouse_drag_color_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragColorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_color(button, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }
}
