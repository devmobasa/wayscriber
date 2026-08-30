//! Color picker popup state methods for InputState.

use std::borrow::Cow;

use crate::draw::Color;
use crate::input::state::InputState;
use crate::input::state::QuickColorEdit;

use super::{
    ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState, PickerDrag,
    color_to_hex, hsv_to_rgb, parse_hex_color, rgb_to_hsv,
};

fn hex_is_complete_for_live_preview(value: &str) -> bool {
    // Six digits is a complete opaque color and eight a complete translucent
    // one. Seven is mid-alpha-pair, so it must not flash a provisional color.
    matches!(value.strip_prefix('#').unwrap_or(value).len(), 6 | 8)
}

/// Upper bound on the characters a title carries out of an authored label. This
/// is not a visual fit — glyph widths vary far too much for a character budget
/// to bound a rendered width, so the renderer trims the shaped title to the
/// panel. The cap only stops a pathological label from making that measurement
/// walk a huge string every frame.
const TITLE_LABEL_MAX_CHARS: usize = 64;

/// Flatten an authored label into one line of title text: a TOML label may hold
/// newlines, tabs, or runs of spaces, none of which a single-line title can
/// render (and a line break would draw straight out of the popup's damage).
fn single_line_slot_label(label: &str) -> String {
    let flattened = label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(TITLE_LABEL_MAX_CHARS)
        .collect::<String>();
    flattened.trim_end().to_string()
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
        self.discard_open_color_picker_recolor();
        let color = self.color_for_tool(self.active_tool());
        self.open_color_picker_popup_for(None, color);
    }

    /// Opens the color picker popup bound to a quick-color slot, so editing it
    /// recolors that swatch instead of the tool's color. Returns false when the
    /// index is past the palette — a stale click on a snapshot rendered before
    /// the palette shrank, which must open nothing.
    pub fn open_color_picker_popup_for_quick_color(&mut self, index: usize) -> bool {
        if self.quick_colors.entry(index).is_none() {
            return false;
        }
        // Abandon a live recolor first, so the color read below is the slot's
        // saved value even when the same swatch is right-clicked twice.
        self.discard_open_color_picker_recolor();
        let Some(color) = self.quick_colors.color_for_index(index) else {
            return false;
        };
        self.open_color_picker_popup_for(Some(index), color);
        true
    }

    /// Reopening the picker — a second swatch, or the tool's own color chip —
    /// abandons an open recolor, so revert its live palette change instead of
    /// dropping the state and leaving the swatch changed but unsaved. A
    /// tool-color preview is left in place, as reopening has always done.
    fn discard_open_color_picker_recolor(&mut self) {
        if self.color_picker_popup_slot().is_some() {
            self.close_color_picker_popup(true);
        }
    }

    fn open_color_picker_popup_for(&mut self, slot: Option<usize>, color: Color) {
        self.cancel_pending_color_picker_paste();
        self.close_modals_for_open(crate::input::state::core::modal::ModalSurface::ColorPicker);
        self.cancel_active_interaction();

        let tool = self.active_tool();
        let hex = color_to_hex(color);

        self.color_picker_popup_generation = self.color_picker_popup_generation.wrapping_add(1);
        self.color_picker_popup_pressed_action = None;

        self.color_picker_popup_state = ColorPickerPopupState::Open {
            tool,
            slot,
            original_color: color,
            current_color: color,
            hex_editing: false,
            hex_buffer: hex,
            dragging: None,
            picker_hsv: rgb_to_hsv(color.r, color.g, color.b),
            hex_selected: false,
            hover_pos: None,
        };

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Title for the open popup: the slot being recolored is named so the
    /// target is never ambiguous. This is the semantic title; the renderer
    /// trims the shaped text to the panel it draws into.
    pub fn color_picker_popup_title(&self) -> Cow<'static, str> {
        let ColorPickerPopupState::Open {
            slot: Some(index), ..
        } = &self.color_picker_popup_state
        else {
            return Cow::Borrowed("Select Color");
        };
        match self.quick_colors.entry(*index) {
            Some(entry) => Cow::Owned(format!("Recolor {}", single_line_slot_label(&entry.label))),
            None => Cow::Borrowed("Recolor swatch"),
        }
    }

    /// The quick-color slot the open popup edits, if any.
    pub fn color_picker_popup_slot(&self) -> Option<usize> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { slot, .. } => *slot,
            ColorPickerPopupState::Hidden => None,
        }
    }

    /// Hand an accepted recolor to the backend, which owns the config and the
    /// file.
    ///
    /// Accepting is an explicit user edit action, so the slot is written to
    /// `config.toml`. `InputState` has neither the configuration nor the
    /// filesystem, so the accept records what it decided and the backend drains
    /// it — and raises the toast, which depends on whether the write landed.
    fn request_quick_color_edit(&mut self, index: usize, color: Color) {
        self.emit_input_effect(super::super::base::InputEffect::QuickColor(
            QuickColorEdit { index, color },
        ));
    }

    /// Show a candidate color on the popup's edit target: the swatch it
    /// recolors, or the tool's color. Live palette updates keep the toolbar
    /// swatch in step with the gradient drag; they are reverted on cancel and
    /// persisted on accept.
    fn color_picker_popup_preview(&mut self, color: Color) {
        match self.color_picker_popup_state {
            ColorPickerPopupState::Open {
                slot: Some(index), ..
            } => {
                if self.quick_colors.set_color_for_index(index, color) {
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
            }
            ColorPickerPopupState::Open {
                tool, slot: None, ..
            } => {
                let _ = self.preview_color_for_tool(tool, color);
            }
            ColorPickerPopupState::Hidden => {}
        }
    }

    /// Closes the color picker popup, optionally restoring the original color.
    pub fn close_color_picker_popup(&mut self, restore_original: bool) {
        self.cancel_pending_color_picker_paste();
        let mut restored_color = None;
        if let ColorPickerPopupState::Open {
            slot,
            original_color,
            ..
        } = &self.color_picker_popup_state
            // A recolor edits durable config, so even an implicit close (light
            // mode, session restore) must not leave the palette changed and
            // unsaved. A tool-color preview stays put, as callers expect.
            && (restore_original || slot.is_some())
        {
            restored_color = Some(*original_color);
        }
        if let Some(color) = restored_color {
            // Restores whichever target the popup was editing, so a cancelled
            // recolor puts the swatch's own color back.
            self.color_picker_popup_preview(color);
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
            slot,
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
            applied_color = Some((*tool, *slot, *original_color, *current_color));
        }
        if let Some((tool, slot, original_color, color)) = applied_color
            && original_color != color
        {
            // Commit on the popup's own target, which also catches a
            // three-digit hex first parsed just above.
            self.color_picker_popup_preview(color);
            match slot {
                Some(index) => {
                    self.request_quick_color_edit(index, color);
                    // The swatch the tool was already painting with follows its
                    // own recolor, so the palette's selection ring and the live
                    // color cannot disagree. Recoloring any other slot leaves
                    // the current color alone.
                    if self.color_for_tool(tool) == original_color {
                        let _ = self.preview_color_for_tool(tool, color);
                        self.active_preset_slot = None;
                        self.note_recent_color(color);
                        self.mark_session_dirty();
                    }
                }
                None => {
                    self.active_preset_slot = None;
                    // Accepting is where a mixed color becomes the color in
                    // use, so it belongs in recents. This commits on the
                    // popup's own target rather than through
                    // `apply_color_from_ui`, which is what records every other
                    // UI color source.
                    self.note_recent_color(color);
                    self.mark_session_dirty();
                }
            }
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
        self.discard_pending_color_picker_paste();
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
        self.color_picker_popup_layout = Some(ColorPickerPopupLayout::compute(
            screen_width,
            screen_height,
            self.color_picker_popup_shows_default_button(),
        ));
    }

    /// Whether the popup offers its "Default" button: only while recoloring a
    /// quick-color slot the shipped palette defines. Slots a user added past
    /// the built-in palette have no default to restore, and the tool-color
    /// popup edits a value the palette does not own.
    pub fn color_picker_popup_shows_default_button(&self) -> bool {
        self.color_picker_popup_default_color().is_some()
    }

    /// The built-in color of the slot being recolored, when there is one.
    pub fn color_picker_popup_default_color(&self) -> Option<Color> {
        let index = self.color_picker_popup_slot()?;
        crate::config::default_quick_color_for_index(index)
    }

    /// Load the slot's built-in color as the live candidate. The popup stays
    /// open, so the change still goes through OK (or backs out via Cancel)
    /// like any other pick. Returns false when there is no default to restore.
    pub fn color_picker_popup_restore_default(&mut self) -> bool {
        let Some(color) = self.color_picker_popup_default_color() else {
            return false;
        };
        self.color_picker_popup_set_color(color);
        true
    }

    /// Clears the cached color picker popup layout.
    pub fn clear_color_picker_popup_layout(&mut self) {
        self.color_picker_popup_layout = None;
    }

    /// Sets the current color from a position in the saturation/value square.
    ///
    /// `norm_x` is saturation and `norm_y` runs from full value at the top to
    /// black at the bottom. Hue comes from the remembered triple rather than
    /// the pointer, so the square and the hue bar stay independent.
    pub fn color_picker_popup_set_from_gradient(&mut self, norm_x: f64, norm_y: f64) {
        let saturation = norm_x.clamp(0.0, 1.0);
        let value = (1.0 - norm_y).clamp(0.0, 1.0);
        let hue = self.color_picker_popup_hsv().map_or(0.0, |(h, _, _)| h);
        let color = self.color_picker_popup_with_alpha(hsv_to_rgb(hue, saturation, value));
        self.color_picker_popup_remember_hsv((hue, saturation, value));
        self.color_picker_popup_set_color_internal(color);
    }

    /// Re-applies the live alpha to a color rebuilt from HSV.
    ///
    /// `hsv_to_rgb` always returns an opaque color, so without this every drag
    /// on the square or the hue bar would silently reset a translucent color to
    /// fully opaque.
    fn color_picker_popup_with_alpha(&self, color: Color) -> Color {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { current_color, .. } => Color {
                a: current_color.a,
                ..color
            },
            ColorPickerPopupState::Hidden => color,
        }
    }

    /// Sets the color's alpha from a position on the alpha bar.
    pub fn color_picker_popup_set_alpha(&mut self, norm_x: f64) {
        let alpha = norm_x.clamp(0.0, 1.0);
        let ColorPickerPopupState::Open { current_color, .. } = &self.color_picker_popup_state
        else {
            return;
        };
        let color = Color {
            a: alpha,
            ..*current_color
        };
        self.color_picker_popup_set_color_internal(color);
    }

    /// The live color's alpha, if the popup is open.
    pub fn color_picker_popup_alpha(&self) -> Option<f64> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { current_color, .. } => Some(current_color.a),
            ColorPickerPopupState::Hidden => None,
        }
    }

    /// Commits a color the picker computed itself: updates the live color and
    /// hex buffer and previews it on the edit target. Shared by the
    /// saturation/value square and the hue bar so they cannot drift.
    fn color_picker_popup_set_color_internal(&mut self, color: Color) {
        let mut live_color = None;
        if let ColorPickerPopupState::Open {
            current_color,
            hex_buffer,
            ..
        } = &mut self.color_picker_popup_state
        {
            *current_color = color;
            *hex_buffer = color_to_hex(color);
            live_color = Some(color);
        }
        if let Some(color) = live_color {
            self.color_picker_popup_preview(color);
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
            live_color = Some(color);
        }
        if let Some(color) = live_color {
            self.color_picker_popup_preview(color);
        }
        self.needs_redraw = true;
    }

    /// Records which picker area a drag is steering, or `None` to end it.
    pub fn color_picker_popup_set_dragging(&mut self, dragging: Option<PickerDrag>) {
        if let ColorPickerPopupState::Open {
            dragging: drag_state,
            ..
        } = &mut self.color_picker_popup_state
        {
            *drag_state = dragging;
        }
    }

    /// Whether any picker drag is in flight.
    pub fn color_picker_popup_is_dragging(&self) -> bool {
        self.color_picker_popup_drag_target().is_some()
    }

    /// Takes the in-flight drag target, ending the drag.
    pub(in crate::input::state) fn color_picker_popup_take_drag_target(
        &mut self,
    ) -> Option<PickerDrag> {
        let target = self.color_picker_popup_drag_target();
        if target.is_some() {
            self.color_picker_popup_set_dragging(None);
        }
        target
    }

    /// Steers one picker control from a pointer position.
    ///
    /// Shared by press, drag-motion and release so all three read the pointer
    /// the same way: whichever control the gesture started on keeps steering,
    /// even once the pointer leaves that control's bounds.
    pub(crate) fn color_picker_popup_apply_drag(&mut self, target: PickerDrag, x: f64, y: f64) {
        let Some(layout) = self.color_picker_popup_layout() else {
            return;
        };
        match target {
            PickerDrag::SatVal => {
                let (saturation, value) = layout.sv_from_point(x, y);
                self.color_picker_popup_set_from_gradient(saturation, 1.0 - value);
            }
            PickerDrag::Hue => self.color_picker_popup_set_hue(layout.hue_from_point(x)),
            PickerDrag::Alpha => self.color_picker_popup_set_alpha(layout.alpha_from_point(x)),
        }
    }

    /// The picker area a drag is steering, if one is in flight.
    pub fn color_picker_popup_drag_target(&self) -> Option<PickerDrag> {
        match &self.color_picker_popup_state {
            ColorPickerPopupState::Open { dragging, .. } => *dragging,
            ColorPickerPopupState::Hidden => None,
        }
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

            // Eight digits so an alpha pair can be typed, plus the # prefix.
            let max_len = if hex_buffer.starts_with('#') { 9 } else { 8 };
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
                    live_color = Some(color);
                }
            }
        }
        if let Some(color) = live_color {
            self.color_picker_popup_preview(color);
        }
    }

    /// Removes the last character from the hex input buffer.
    pub fn color_picker_popup_hex_backspace(&mut self) {
        let mut live_color = None;
        {
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

                // Keep the last complete preview while the user edits an
                // incomplete value; Enter still accepts three-digit hex.
                if hex_is_complete_for_live_preview(hex_buffer)
                    && let Some(color) = parse_hex_color(hex_buffer)
                {
                    *current_color = color;
                    live_color = Some(color);
                }
            }
        }
        if let Some(color) = live_color {
            self.color_picker_popup_preview(color);
        }
    }

    /// Commits the hex input (parses and applies the color).
    pub fn color_picker_popup_commit_hex(&mut self) -> bool {
        let parsed_color = {
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
                self.needs_redraw = true;
                Some(color)
            } else {
                // Reset buffer to current color
                *hex_buffer = color_to_hex(*current_color);
                *hex_editing = false;
                self.needs_redraw = true;
                None
            }
        };

        if let Some(color) = parsed_color {
            self.color_picker_popup_preview(color);
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

    /// The HSV triple the picker is showing, if it is open.
    ///
    /// Prefers the remembered triple while it still resolves to the current
    /// color. Grey, black and white all convert back to a hue of zero, so
    /// without this the hue bar would jump to red whenever value or saturation
    /// reached an edge.
    pub fn color_picker_popup_hsv(&self) -> Option<(f64, f64, f64)> {
        let ColorPickerPopupState::Open {
            current_color,
            picker_hsv,
            ..
        } = &self.color_picker_popup_state
        else {
            return None;
        };
        let (h, s, v) = *picker_hsv;
        let remembered = hsv_to_rgb(h, s, v);
        if (remembered.r - current_color.r).abs() < 1e-3
            && (remembered.g - current_color.g).abs() < 1e-3
            && (remembered.b - current_color.b).abs() < 1e-3
        {
            return Some((h, s, v));
        }
        Some(rgb_to_hsv(
            current_color.r,
            current_color.g,
            current_color.b,
        ))
    }

    fn color_picker_popup_remember_hsv(&mut self, hsv: (f64, f64, f64)) {
        if let ColorPickerPopupState::Open { picker_hsv, .. } = &mut self.color_picker_popup_state {
            *picker_hsv = hsv;
        }
    }

    /// Sets the current color from a position on the hue bar, keeping
    /// saturation and value where they are.
    pub fn color_picker_popup_set_hue(&mut self, norm_x: f64) {
        let hue = norm_x.clamp(0.0, 1.0);
        let (_, saturation, value) = self.color_picker_popup_hsv().unwrap_or((0.0, 1.0, 1.0));
        let color = self.color_picker_popup_with_alpha(hsv_to_rgb(hue, saturation, value));
        self.color_picker_popup_remember_hsv((hue, saturation, value));
        self.color_picker_popup_set_color_internal(color);
    }

    /// Position within the saturation/value square for the current color.
    pub fn color_picker_popup_gradient_position(&self) -> Option<(f64, f64)> {
        let (_, saturation, value) = self.color_picker_popup_hsv()?;
        Some((saturation, 1.0 - value))
    }

    /// Position along the hue bar for the current color.
    pub fn color_picker_popup_hue_position(&self) -> Option<f64> {
        self.color_picker_popup_hsv().map(|(hue, _, _)| hue)
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
