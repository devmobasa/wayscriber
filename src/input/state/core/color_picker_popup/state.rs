//! Color picker popup state methods for InputState.

use crate::draw::Color;
use crate::input::state::InputState;

use super::{
    ColorPickerPopupLayout, ColorPickerPopupState, color_to_hex, hsv_to_rgb, parse_hex_color,
    rgb_to_hsv,
};

impl InputState {
    /// Returns true if the color picker popup is open.
    pub fn is_color_picker_popup_open(&self) -> bool {
        matches!(
            self.color_picker_popup_state,
            ColorPickerPopupState::Open { .. }
        )
    }

    /// Opens the color picker popup with the current color.
    pub fn open_color_picker_popup(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_board_picker();

        let color = self.current_color;
        let hex = color_to_hex(color);

        self.color_picker_popup_state = ColorPickerPopupState::Open {
            original_color: color,
            current_color: color,
            hex_editing: false,
            hex_buffer: hex,
            dragging: false,
            hex_selected: false,
            hover_pos: None,
        };

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Closes the color picker popup, optionally restoring the original color.
    pub fn close_color_picker_popup(&mut self, restore_original: bool) {
        if let ColorPickerPopupState::Open { original_color, .. } = &self.color_picker_popup_state
            && restore_original
        {
            self.current_color = *original_color;
            self.sync_highlight_color();
        }
        self.color_picker_popup_state = ColorPickerPopupState::Hidden;
        self.color_picker_popup_layout = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Applies the current color and closes the popup.
    pub fn apply_color_picker_popup(&mut self) {
        if let ColorPickerPopupState::Open { current_color, .. } = &self.color_picker_popup_state {
            self.current_color = *current_color;
            self.sync_highlight_color();
        }
        self.color_picker_popup_state = ColorPickerPopupState::Hidden;
        self.color_picker_popup_layout = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Gets the current color in the popup (if open).
    pub fn color_picker_popup_current_color(&self) -> Option<Color> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { current_color, .. } => Some(*current_color),
            ColorPickerPopupState::Hidden => None,
        }
    }

    /// Gets the cached layout for the color picker popup.
    pub fn color_picker_popup_layout(&self) -> Option<ColorPickerPopupLayout> {
        self.color_picker_popup_layout
    }

    /// Updates the layout for the color picker popup.
    pub fn update_color_picker_popup_layout(&mut self, screen_width: u32, screen_height: u32) {
        if !self.is_color_picker_popup_open() {
            self.color_picker_popup_layout = None;
            return;
        }
        self.color_picker_popup_layout =
            Some(ColorPickerPopupLayout::compute(screen_width, screen_height));
    }

    /// Clears the cached color picker popup layout.
    pub fn clear_color_picker_popup_layout(&mut self) {
        self.color_picker_popup_layout = None;
    }

    /// Sets the current color from gradient coordinates.
    pub fn color_picker_popup_set_from_gradient(&mut self, norm_x: f64, norm_y: f64) {
        let hue = norm_x.clamp(0.0, 1.0);
        let value = (1.0 - norm_y).clamp(0.0, 1.0);
        let color = hsv_to_rgb(hue, 1.0, value);

        if let ColorPickerPopupState::Open {
            current_color,
            hex_buffer,
            ..
        } = &mut self.color_picker_popup_state
        {
            *current_color = color;
            *hex_buffer = color_to_hex(color);
            // Live update the drawing color
            self.current_color = color;
            self.sync_highlight_color();
        }
        self.needs_redraw = true;
    }

    /// Updates whether we're dragging on the gradient.
    pub fn color_picker_popup_set_dragging(&mut self, dragging: bool) {
        if let ColorPickerPopupState::Open {
            dragging: drag_state,
            ..
        } = &mut self.color_picker_popup_state
        {
            *drag_state = dragging;
        }
    }

    /// Returns true if we're currently dragging on the gradient.
    pub fn color_picker_popup_is_dragging(&self) -> bool {
        matches!(
            &self.color_picker_popup_state,
            ColorPickerPopupState::Open { dragging: true, .. }
        )
    }

    /// Sets whether the hex input field is focused.
    pub fn color_picker_popup_set_hex_editing(&mut self, editing: bool) {
        if let ColorPickerPopupState::Open {
            hex_editing,
            hex_buffer,
            hex_selected,
            current_color,
            ..
        } = &mut self.color_picker_popup_state
        {
            *hex_editing = editing;
            // When starting to edit, ensure buffer matches current color and select all
            if editing {
                *hex_buffer = color_to_hex(*current_color);
                *hex_selected = true; // Auto-select so first keystroke replaces
            } else {
                *hex_selected = false;
            }
        }
        self.needs_redraw = true;
    }

    /// Returns true if the hex input is currently being edited.
    pub fn color_picker_popup_is_hex_editing(&self) -> bool {
        matches!(
            &self.color_picker_popup_state,
            ColorPickerPopupState::Open {
                hex_editing: true,
                ..
            }
        )
    }

    /// Returns true if the hex input text is currently selected (replace-on-type).
    pub fn color_picker_popup_hex_selected(&self) -> bool {
        matches!(
            &self.color_picker_popup_state,
            ColorPickerPopupState::Open {
                hex_selected: true,
                ..
            }
        )
    }

    /// Appends a character to the hex input buffer.
    pub fn color_picker_popup_hex_append(&mut self, ch: char) {
        let ColorPickerPopupState::Open {
            hex_buffer,
            hex_editing,
            hex_selected,
            current_color,
            ..
        } = &mut self.color_picker_popup_state
        else {
            return;
        };

        if !*hex_editing {
            return;
        }

        // If text is selected, first keystroke clears the buffer (replaces all)
        if *hex_selected {
            hex_buffer.clear();
            *hex_selected = false;
        }

        // Handle # prefix
        if ch == '#' && hex_buffer.is_empty() {
            hex_buffer.push(ch);
            self.needs_redraw = true;
            return;
        }

        // Max length is 7 with # prefix or 6 without
        let max_len = if hex_buffer.starts_with('#') { 7 } else { 6 };
        if hex_buffer.len() >= max_len {
            return;
        }

        // Only allow hex digits
        if ch.is_ascii_hexdigit() {
            hex_buffer.push(ch.to_ascii_uppercase());
            self.needs_redraw = true;

            // Try to parse and update color live
            if let Some(color) = parse_hex_color(hex_buffer) {
                *current_color = color;
                // Live update the drawing color
                self.current_color = color;
                self.sync_highlight_color();
            }
        }
    }

    /// Removes the last character from the hex input buffer.
    pub fn color_picker_popup_hex_backspace(&mut self) {
        if let ColorPickerPopupState::Open {
            hex_buffer,
            hex_editing,
            hex_selected,
            current_color,
            ..
        } = &mut self.color_picker_popup_state
            && *hex_editing
        {
            // If text is selected, backspace clears all
            if *hex_selected {
                hex_buffer.clear();
                *hex_selected = false;
            } else if !hex_buffer.is_empty() {
                hex_buffer.pop();
            }
            self.needs_redraw = true;

            // Try to parse and update color live
            if let Some(color) = parse_hex_color(hex_buffer) {
                *current_color = color;
                self.current_color = color;
                self.sync_highlight_color();
            }
        }
    }

    /// Commits the hex input (parses and applies the color).
    pub fn color_picker_popup_commit_hex(&mut self) -> bool {
        let ColorPickerPopupState::Open {
            hex_buffer,
            hex_editing,
            current_color,
            ..
        } = &mut self.color_picker_popup_state
        else {
            return false;
        };

        if !*hex_editing {
            return false;
        }

        if let Some(color) = parse_hex_color(hex_buffer) {
            *current_color = color;
            *hex_buffer = color_to_hex(color);
            *hex_editing = false;
            // Live update the drawing color
            self.current_color = color;
            self.sync_highlight_color();
            self.needs_redraw = true;
            true
        } else {
            // Reset buffer to current color
            *hex_buffer = color_to_hex(*current_color);
            *hex_editing = false;
            self.needs_redraw = true;
            false
        }
    }

    /// Gets the current hex buffer value.
    pub fn color_picker_popup_hex_buffer(&self) -> Option<&str> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { hex_buffer, .. } => Some(hex_buffer.as_str()),
            ColorPickerPopupState::Hidden => None,
        }
    }

    /// Returns true if the current hex buffer is valid (or empty/in-progress).
    pub fn color_picker_popup_hex_valid(&self) -> bool {
        let Some(hex_buffer) = self.color_picker_popup_hex_buffer() else {
            return true;
        };
        parse_hex_color(hex_buffer).is_some() || hex_buffer.is_empty() || hex_buffer == "#"
    }

    /// Gets the gradient position for the current color.
    pub fn color_picker_popup_gradient_position(&self) -> Option<(f64, f64)> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { current_color, .. } => {
                let (hue, _, value) = rgb_to_hsv(current_color.r, current_color.g, current_color.b);
                Some((hue, 1.0 - value))
            }
            ColorPickerPopupState::Hidden => None,
        }
    }

    /// Sets the hover position within the popup.
    #[allow(dead_code)]
    pub fn color_picker_popup_set_hover(&mut self, pos: Option<(f64, f64)>) {
        if let ColorPickerPopupState::Open { hover_pos, .. } = &mut self.color_picker_popup_state {
            *hover_pos = pos;
        }
    }

    /// Gets the current hover position within the popup.
    pub fn color_picker_popup_hover(&self) -> Option<(f64, f64)> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { hover_pos, .. } => *hover_pos,
            ColorPickerPopupState::Hidden => None,
        }
    }
}
