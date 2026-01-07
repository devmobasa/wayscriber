use crate::config::Action;
use log::info;

use super::super::{InputState, SelectionAxis, UiToastKind};

const KEYBOARD_NUDGE_SMALL: i32 = 8;
const KEYBOARD_NUDGE_LARGE: i32 = 32;

impl InputState {
    pub(super) fn handle_selection_action(&mut self, action: Action) -> bool {
        match action {
            Action::CopySelection => {
                let copied = self.copy_selection();
                if copied > 0 {
                    info!("Copied selection ({} shape(s))", copied);
                } else if self.has_selection() {
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        "No unlocked shapes to copy; clipboard unchanged.",
                    );
                } else {
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        "No selection to copy; clipboard unchanged.",
                    );
                }
                true
            }
            Action::PasteSelection => {
                let pasted = self.paste_selection();
                if pasted > 0 {
                    info!("Pasted selection ({} shape(s))", pasted);
                } else if self.selection_clipboard_is_empty() {
                    self.set_ui_toast(UiToastKind::Warning, "Clipboard is empty.");
                    self.trigger_blocked_feedback();
                }
                true
            }
            Action::SelectAll => {
                let previous_bounds = self.selection_bounding_box(self.selected_shape_ids());
                let ids: Vec<_> = self
                    .canvas_set
                    .active_frame()
                    .shapes
                    .iter()
                    .map(|shape| shape.id)
                    .collect();
                if ids.is_empty() {
                    self.set_ui_toast(UiToastKind::Warning, "No shapes to select.");
                } else {
                    self.set_selection(ids);
                    self.mark_selection_dirty_region(previous_bounds);
                    let new_bounds = self.selection_bounding_box(self.selected_shape_ids());
                    self.mark_selection_dirty_region(new_bounds);
                    self.needs_redraw = true;
                }
                true
            }
            Action::DuplicateSelection => {
                if self.duplicate_selection() {
                    info!("Duplicated selection");
                }
                true
            }
            Action::MoveSelectionToFront => {
                if self.move_selection_to_front() {
                    info!("Moved selection to front");
                }
                true
            }
            Action::MoveSelectionToBack => {
                if self.move_selection_to_back() {
                    info!("Moved selection to back");
                }
                true
            }
            Action::NudgeSelectionUp => {
                let step = if self.modifiers.shift {
                    KEYBOARD_NUDGE_LARGE
                } else {
                    KEYBOARD_NUDGE_SMALL
                };
                if self.translate_selection_with_undo(0, -step) {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                    info!("Moved selection up by {} px", step);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                }
                true
            }
            Action::NudgeSelectionDown => {
                let step = if self.modifiers.shift {
                    KEYBOARD_NUDGE_LARGE
                } else {
                    KEYBOARD_NUDGE_SMALL
                };
                if self.translate_selection_with_undo(0, step) {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                    info!("Moved selection down by {} px", step);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                }
                true
            }
            Action::NudgeSelectionLeft => {
                let step = if self.modifiers.shift {
                    KEYBOARD_NUDGE_LARGE
                } else {
                    KEYBOARD_NUDGE_SMALL
                };
                if self.translate_selection_with_undo(-step, 0) {
                    self.last_selection_axis = Some(SelectionAxis::Horizontal);
                    info!("Moved selection left by {} px", step);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Horizontal);
                }
                true
            }
            Action::NudgeSelectionRight => {
                let step = if self.modifiers.shift {
                    KEYBOARD_NUDGE_LARGE
                } else {
                    KEYBOARD_NUDGE_SMALL
                };
                if self.translate_selection_with_undo(step, 0) {
                    self.last_selection_axis = Some(SelectionAxis::Horizontal);
                    info!("Moved selection right by {} px", step);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Horizontal);
                }
                true
            }
            Action::NudgeSelectionUpLarge => {
                if self.translate_selection_with_undo(0, -KEYBOARD_NUDGE_LARGE) {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                    info!("Moved selection up by {} px", KEYBOARD_NUDGE_LARGE);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                }
                true
            }
            Action::NudgeSelectionDownLarge => {
                if self.translate_selection_with_undo(0, KEYBOARD_NUDGE_LARGE) {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                    info!("Moved selection down by {} px", KEYBOARD_NUDGE_LARGE);
                } else if self.has_selection() {
                    self.last_selection_axis = Some(SelectionAxis::Vertical);
                }
                true
            }
            Action::MoveSelectionToStart => {
                if self.move_selection_to_horizontal_edge(true) {
                    info!("Moved selection to start");
                }
                true
            }
            Action::MoveSelectionToEnd => {
                if self.move_selection_to_horizontal_edge(false) {
                    info!("Moved selection to end");
                }
                true
            }
            Action::MoveSelectionToTop => {
                if self.move_selection_to_vertical_edge(true) {
                    info!("Moved selection to top");
                }
                true
            }
            Action::MoveSelectionToBottom => {
                if self.move_selection_to_vertical_edge(false) {
                    info!("Moved selection to bottom");
                }
                true
            }
            Action::DeleteSelection => {
                if self.delete_selection() {
                    info!("Deleted selection");
                }
                true
            }
            _ => false,
        }
    }
}
