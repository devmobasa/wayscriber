use crate::input::events::MouseButton;
use crate::input::state::core::PickerDrag;

use super::super::super::InputState;

impl InputState {
    fn is_point_in_context_menu(&self, x: i32, y: i32) -> bool {
        self.context_menu_level_at(x, y).is_some()
    }

    pub(in crate::input::state) fn handle_context_menu_press(
        &mut self,
        screen_x: i32,
        screen_y: i32,
    ) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }

        self.text_editing.set_last_click(None);
        if self.is_point_in_context_menu(screen_x, screen_y) {
            self.update_context_menu_hover_from_pointer(screen_x, screen_y);
        } else {
            self.close_context_menu();
            self.needs_redraw = true;
        }
        true
    }

    pub(in crate::input::state) fn handle_radial_menu_press_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        button: MouseButton,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) -> bool {
        if !self.is_radial_menu_open() {
            return false;
        }
        self.update_pointer_positions(screen_x, screen_y, canvas_x, canvas_y);
        match button {
            MouseButton::Left => {
                // Update hover at exact click position before selecting
                self.update_radial_menu_hover(screen_x as f64, screen_y as f64);
                if self.radial_menu_hover_is_size_ring() {
                    // Pressing the size gauge starts a drag-capture along the
                    // arc instead of selecting.
                    self.radial_menu_begin_size_drag_with_measurer(
                        resources.measurer,
                        screen_x as f64,
                        screen_y as f64,
                    );
                } else {
                    self.radial_menu_select_hovered_with_resources(resources);
                }
            }
            MouseButton::Right => {
                self.close_radial_menu();
                if !self.is_radial_menu_toggle_button(MouseButton::Right) {
                    // Keep right-click context-menu flow when right button is not the
                    // configured radial-menu trigger.
                    self.handle_right_click_with_measurer(
                        resources.measurer,
                        screen_x,
                        screen_y,
                        canvas_x,
                        canvas_y,
                    );
                }
            }
            MouseButton::Middle => {
                self.close_radial_menu();
            }
        }
        true
    }

    pub(in crate::input::state) fn handle_color_picker_press(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        if !self.is_color_picker_popup_open() {
            return false;
        }
        self.update_pointer_position(x, y);
        match button {
            MouseButton::Left => {
                let action_pressed = self.color_picker_popup_note_action_press(x, y);
                if !action_pressed && let Some(layout) = self.color_picker_popup_layout() {
                    let fx = x as f64;
                    let fy = y as f64;
                    let target = if layout.point_in_sv(fx, fy) {
                        Some(PickerDrag::SatVal)
                    } else if layout.point_in_hue(fx, fy) {
                        Some(PickerDrag::Hue)
                    } else if layout.point_in_alpha(fx, fy) {
                        Some(PickerDrag::Alpha)
                    } else {
                        None
                    };
                    if let Some(target) = target {
                        self.color_picker_popup_set_dragging(Some(target));
                        self.color_picker_popup_apply_drag(target, fx, fy);
                        self.color_picker_popup_set_hex_editing(false);
                    }
                }
            }
            MouseButton::Right => {
                self.color_picker_popup_clear_action_press();
                self.close_color_picker_popup(true);
            }
            MouseButton::Middle => self.color_picker_popup_clear_action_press(),
        }
        true
    }

    pub(in crate::input::state) fn handle_board_picker_press(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        if !self.is_board_picker_open() {
            return false;
        }
        if self.board_appearance_edit().is_some() {
            // The size slider acts on press so it can be dragged.
            if matches!(button, MouseButton::Left) {
                self.board_appearance_press(x, y);
            }
            return true;
        }
        self.update_pointer_position(x, y);
        match button {
            MouseButton::Left => {
                if self.board_picker_contains_point(x, y) {
                    if let Some(index) = self.board_picker_page_handle_index_at(x, y) {
                        self.board_picker_start_page_drag(index);
                        return true;
                    }
                    if let Some(row) = self.board_picker_handle_index_at(x, y) {
                        self.board_picker_start_drag(row);
                        return true;
                    }
                    if self.board_picker_index_at(x, y).is_some() {
                        self.update_board_picker_hover_from_pointer(x, y);
                    }
                } else {
                    self.close_board_picker();
                }
            }
            MouseButton::Right => {
                if !self.board_picker_contains_point(x, y) {
                    self.close_board_picker();
                } else if let Some(row) = self.board_picker_index_at(x, y)
                    && !self.board_picker_is_new_row(row)
                    && let Some(board_index) = self.board_picker_board_index_for_row(row)
                {
                    // Like a left click, a right click selects the row it acts on.
                    self.board_picker_set_selected(row);
                    self.update_pointer_position_synthetic(x, y);
                    self.open_board_context_menu((x, y), board_index);
                } else if let Some(page_index) = self.board_picker_page_index_at(x, y)
                    && let Some(board_index) = self.board_picker_page_panel_board_index()
                {
                    self.update_pointer_position_synthetic(x, y);
                    self.open_page_context_menu((x, y), board_index, page_index);
                } else {
                    self.close_board_picker();
                }
            }
            MouseButton::Middle => {}
        }
        true
    }

    pub(in crate::input::state) fn handle_properties_panel_press(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
    ) -> bool {
        if !self.is_properties_panel_open() {
            return false;
        }
        self.update_pointer_position(x, y);
        if self.properties_panel_layout().is_none() {
            return true;
        }
        match button {
            MouseButton::Left => {
                if let Some(index) = self.properties_panel_index_at(x, y) {
                    self.set_properties_panel_focus(Some(index));
                } else {
                    self.close_properties_panel();
                }
            }
            MouseButton::Right => {
                self.close_properties_panel();
            }
            MouseButton::Middle => {}
        }
        true
    }
}
