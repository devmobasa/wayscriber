use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use log::info;

use super::super::{InputState, SelectionAxis};

const KEYBOARD_NUDGE_SMALL: i32 = 8;
const KEYBOARD_NUDGE_LARGE: i32 = 32;

impl InputState {
    pub(in crate::input::state) fn handle_selection_action(&mut self, action: Action) -> bool {
        match action {
            Action::CopySelection | Action::SelectAll => {
                self.handle_selection_content_action(action)
            }
            Action::PasteSelection => {
                self.request_clipboard_paste();
                info!("Requested clipboard paste");
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
            Action::NudgeSelectionUp
            | Action::NudgeSelectionDown
            | Action::NudgeSelectionLeft
            | Action::NudgeSelectionRight
            | Action::NudgeSelectionUpLarge
            | Action::NudgeSelectionDownLarge => self.handle_selection_nudge_action(action),
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

    fn handle_selection_content_action(&mut self, action: Action) -> bool {
        match action {
            Action::CopySelection => {
                let copied = self.copy_selection();
                if copied > 0 {
                    info!("Copied selection ({} shape(s))", copied);
                } else if self.has_selection() {
                    self.push_toast(
                        ToastPriority::Info,
                        "selection",
                        Toast::warning("No unlocked shapes to copy; clipboard unchanged."),
                    );
                } else {
                    self.push_toast(
                        ToastPriority::Info,
                        "selection",
                        Toast::warning("No selection to copy; clipboard unchanged."),
                    );
                }
            }
            Action::SelectAll => {
                crate::draw::with_legacy_measurer(|measurer| self.select_all_shapes_with(measurer));
            }
            _ => unreachable!("selection content dispatcher called with {action:?}"),
        }
        true
    }

    fn select_all_shapes_with(&mut self, measurer: &crate::draw::TextMeasurer) {
        let previous_bounds = self.selection_bounding_box_with(measurer, self.selected_shape_ids());
        let ids: Vec<_> = self
            .boards
            .active_frame()
            .shapes
            .iter()
            .map(|shape| shape.id)
            .collect();
        if ids.is_empty() {
            self.push_toast(
                ToastPriority::Info,
                "selection",
                Toast::warning("No shapes to select."),
            );
        } else {
            self.set_selection(ids);
            self.mark_selection_dirty_region(previous_bounds);
            let new_bounds = self.selection_bounding_box_with(measurer, self.selected_shape_ids());
            self.mark_selection_dirty_region(new_bounds);
            self.needs_redraw = true;
        }
    }

    fn handle_selection_nudge_action(&mut self, action: Action) -> bool {
        let shifted_step = if self.modifiers.shift {
            KEYBOARD_NUDGE_LARGE
        } else {
            KEYBOARD_NUDGE_SMALL
        };
        let (dx, dy, axis, direction, step) = match action {
            Action::NudgeSelectionUp => (
                0,
                -shifted_step,
                SelectionAxis::Vertical,
                "up",
                shifted_step,
            ),
            Action::NudgeSelectionDown => (
                0,
                shifted_step,
                SelectionAxis::Vertical,
                "down",
                shifted_step,
            ),
            Action::NudgeSelectionLeft => (
                -shifted_step,
                0,
                SelectionAxis::Horizontal,
                "left",
                shifted_step,
            ),
            Action::NudgeSelectionRight => (
                shifted_step,
                0,
                SelectionAxis::Horizontal,
                "right",
                shifted_step,
            ),
            Action::NudgeSelectionUpLarge => (
                0,
                -KEYBOARD_NUDGE_LARGE,
                SelectionAxis::Vertical,
                "up",
                KEYBOARD_NUDGE_LARGE,
            ),
            Action::NudgeSelectionDownLarge => (
                0,
                KEYBOARD_NUDGE_LARGE,
                SelectionAxis::Vertical,
                "down",
                KEYBOARD_NUDGE_LARGE,
            ),
            _ => unreachable!("selection nudge dispatcher called with {action:?}"),
        };
        if self.translate_selection_with_undo(dx, dy) {
            self.selection_interaction.note_axis(axis);
            info!("Moved selection {} by {} px", direction, step);
        } else if self.has_selection() {
            self.selection_interaction.note_axis(axis);
        }
        true
    }
}
