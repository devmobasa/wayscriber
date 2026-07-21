//! Color picker popup state methods for InputState.

use crate::draw::Color;
use crate::input::state::InputState;

use super::{
    ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState, HexPasteTarget,
    color_to_hex, hsv_to_rgb, parse_hex_color, rgb_to_hsv,
};

fn hex_is_complete_for_live_preview(value: &str) -> bool {
    value.strip_prefix('#').unwrap_or(value).len() == 6
}

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
        self.cancel_pending_color_picker_paste();
        self.close_radial_menu();
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_board_picker();

        let tool = self.active_tool();
        let color = self.color_for_tool(tool);
        let hex = color_to_hex(color);

        self.color_picker_popup_generation = self.color_picker_popup_generation.wrapping_add(1);
        self.color_picker_popup_pressed_action = None;

        self.color_picker_popup_state = ColorPickerPopupState::Open {
            tool,
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
        self.cancel_pending_color_picker_paste();
        let mut restored_color = None;
        if let ColorPickerPopupState::Open {
            tool,
            original_color,
            ..
        } = &self.color_picker_popup_state
            && restore_original
        {
            restored_color = Some((*tool, *original_color));
        }
        if let Some((tool, color)) = restored_color {
            let _ = self.preview_color_for_tool(tool, color);
        }
        self.color_picker_popup_state = ColorPickerPopupState::Hidden;
        self.color_picker_popup_layout = None;
        self.color_picker_popup_pressed_action = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Applies the current color and closes the popup.
    pub fn apply_color_picker_popup(&mut self) {
        self.cancel_pending_color_picker_paste();
        let mut applied_color = None;
        if let ColorPickerPopupState::Open {
            tool,
            original_color,
            current_color,
            hex_buffer,
            ..
        } = &mut self.color_picker_popup_state
        {
            // Applying is also a commit boundary for valid buffered input.
            // Three-digit hex is intentionally not previewed while typing, so
            // it must be parsed here before the popup closes. Avoid reparsing
            // a synchronized display value because that would quantize exact
            // gradient colors through their eight-bit hex representation.
            let current_hex = color_to_hex(*current_color);
            let buffered_digits = hex_buffer.strip_prefix('#').unwrap_or(hex_buffer);
            let current_digits = current_hex.strip_prefix('#').unwrap_or(&current_hex);
            if !buffered_digits.eq_ignore_ascii_case(current_digits)
                && let Some(color) = parse_hex_color(hex_buffer)
            {
                *current_color = color;
            }
            applied_color = Some((*tool, *original_color, *current_color));
        }
        if let Some((tool, original_color, color)) = applied_color
            && original_color != color
        {
            let _ = self.preview_color_for_tool(tool, color);
            self.active_preset_slot = None;
            self.mark_session_dirty();
        }
        self.color_picker_popup_state = ColorPickerPopupState::Hidden;
        self.color_picker_popup_layout = None;
        self.color_picker_popup_pressed_action = None;
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

    pub(crate) fn color_picker_popup_generation(&self) -> Option<u64> {
        self.is_color_picker_popup_open()
            .then_some(self.color_picker_popup_generation)
    }

    pub(crate) fn color_picker_popup_generation_is_current(&self, generation: u64) -> bool {
        self.color_picker_popup_generation() == Some(generation)
    }

    fn cancel_pending_color_picker_paste(&mut self) {
        if matches!(
            self.pending_paste_hex,
            Some(HexPasteTarget::ColorPickerPopup { .. })
        ) {
            self.pending_paste_hex = None;
        }
    }

    pub(in crate::input::state) fn color_picker_popup_note_action_press(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        let action = self
            .color_picker_popup_layout
            .and_then(|layout| layout.action_at(x as f64, y as f64));
        self.color_picker_popup_pressed_action = action;
        action.is_some()
    }

    pub(in crate::input::state) fn color_picker_popup_clear_action_press(&mut self) {
        self.color_picker_popup_pressed_action = None;
    }

    pub(in crate::input::state) fn color_picker_popup_take_action_press(
        &mut self,
    ) -> Option<ColorPickerPopupAction> {
        self.color_picker_popup_pressed_action.take()
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

        let mut live_color = None;
        if let ColorPickerPopupState::Open {
            tool,
            current_color,
            hex_buffer,
            ..
        } = &mut self.color_picker_popup_state
        {
            *current_color = color;
            *hex_buffer = color_to_hex(color);
            live_color = Some((*tool, color));
        }
        if let Some((tool, color)) = live_color {
            let _ = self.preview_color_for_tool(tool, color);
        }
        self.needs_redraw = true;
    }

    /// Sets the popup's live color directly (e.g. from a pasted hex),
    /// refreshing the hex buffer and previewing on the editing tool. Mirrors
    /// [`Self::color_picker_popup_set_from_gradient`] but takes a color, and
    /// leaves hex editing unfocused so the pasted value shows as the buffer.
    pub fn color_picker_popup_set_color(&mut self, color: Color) {
        let mut live_color = None;
        if let ColorPickerPopupState::Open {
            tool,
            current_color,
            hex_buffer,
            hex_editing,
            hex_selected,
            ..
        } = &mut self.color_picker_popup_state
        {
            *current_color = color;
            *hex_buffer = color_to_hex(color);
            *hex_editing = false;
            *hex_selected = false;
            live_color = Some((*tool, color));
        }
        if let Some((tool, color)) = live_color {
            let _ = self.preview_color_for_tool(tool, color);
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
        let mut live_color = None;
        {
            let ColorPickerPopupState::Open {
                tool,
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

                // Three-digit hex remains valid on commit, but do not flash a
                // provisional shorthand color halfway through a six-digit
                // entry. Live preview only once the full value is present.
                if hex_is_complete_for_live_preview(hex_buffer)
                    && let Some(color) = parse_hex_color(hex_buffer)
                {
                    *current_color = color;
                    live_color = Some((*tool, color));
                }
            }
        }
        if let Some((tool, color)) = live_color {
            let _ = self.preview_color_for_tool(tool, color);
        }
    }

    /// Removes the last character from the hex input buffer.
    pub fn color_picker_popup_hex_backspace(&mut self) {
        let mut live_color = None;
        {
            if let ColorPickerPopupState::Open {
                tool,
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

                // Keep the last complete preview while the user edits an
                // incomplete value; Enter still accepts three-digit hex.
                if hex_is_complete_for_live_preview(hex_buffer)
                    && let Some(color) = parse_hex_color(hex_buffer)
                {
                    *current_color = color;
                    live_color = Some((*tool, color));
                }
            }
        }
        if let Some((tool, color)) = live_color {
            let _ = self.preview_color_for_tool(tool, color);
        }
    }

    /// Commits the hex input (parses and applies the color).
    pub fn color_picker_popup_commit_hex(&mut self) -> bool {
        let parsed_color = {
            let ColorPickerPopupState::Open {
                tool,
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
                self.needs_redraw = true;
                Some((*tool, color))
            } else {
                // Reset buffer to current color
                *hex_buffer = color_to_hex(*current_color);
                *hex_editing = false;
                self.needs_redraw = true;
                None
            }
        };

        if let Some((tool, color)) = parsed_color {
            let _ = self.preview_color_for_tool(tool, color);
            true
        } else {
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
    pub fn color_picker_popup_set_hover(&mut self, pos: Option<(f64, f64)>) {
        let layout = self.color_picker_popup_layout;
        let visual_changed = if let ColorPickerPopupState::Open { hover_pos, .. } =
            &mut self.color_picker_popup_state
        {
            let previous_action =
                layout.and_then(|layout| hover_pos.and_then(|(x, y)| layout.action_at(x, y)));
            let next_action =
                layout.and_then(|layout| pos.and_then(|(x, y)| layout.action_at(x, y)));
            *hover_pos = pos;
            previous_action != next_action
        } else {
            false
        };
        if visual_changed {
            self.needs_redraw = true;
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
