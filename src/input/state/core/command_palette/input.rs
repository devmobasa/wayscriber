use super::super::base::{
    InputState, KeybindingEditOperation, KeybindingEditRequest, PendingBackendAction, UiToastKind,
};
use super::layout;
use super::{
    CommandPaletteCursorHint,
    layout::{CommandPaletteGeometry, CommandPaletteRowAction},
};
use crate::config::{Action, KeyBinding, KeybindingsConfig, action_label};
use crate::input::events::Key;
use crate::input::state::actions::key_press::bindings::key_to_action_label;
use std::time::{Duration, Instant};

const COMMAND_PALETTE_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(280);
#[allow(dead_code)] // Used by the binary Wayland backend; the lib target has no backend modules.
const COMMAND_PALETTE_REPEAT_INTERVAL: Duration = Duration::from_millis(55);

impl InputState {
    pub(crate) fn command_palette_is_engaged(&self) -> bool {
        self.command_palette_open || self.keybinding_capture_action.is_some()
    }

    pub(crate) fn begin_keybinding_capture(&mut self, action: Action) -> bool {
        if KeybindingsConfig::default()
            .bindings_for_action(action)
            .is_none()
        {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "{} has no configurable keyboard shortcut.",
                    action_label(action)
                ),
            );
            return false;
        }
        self.keybinding_capture_action = Some(action);
        self.clear_command_palette_repeat();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn request_keybinding_edit(
        &mut self,
        action: Action,
        operation: KeybindingEditOperation,
    ) -> bool {
        if KeybindingsConfig::default()
            .bindings_for_action(action)
            .is_none()
        {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "{} has no configurable keyboard shortcut.",
                    action_label(action)
                ),
            );
            return false;
        }
        self.set_pending_backend_action(PendingBackendAction::EditKeybinding(
            KeybindingEditRequest { action, operation },
        ));
        true
    }

    fn open_command_palette_internal(&mut self, track_usage: bool) {
        self.command_palette_open = true;
        self.clear_command_palette_repeat();
        if track_usage {
            self.pending_onboarding_usage.used_command_palette = true;
        }
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_scroll = 0;
        // Close other overlays
        if self.show_help {
            self.show_help = false;
        }
        if self.tour_active {
            self.tour_active = false;
        }
        self.close_context_menu();
        self.close_properties_panel();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Toggle the command palette visibility.
    pub(crate) fn toggle_command_palette(&mut self) {
        if self.command_palette_open {
            self.command_palette_open = false;
            self.clear_command_palette_repeat();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }
        self.open_command_palette_internal(true);
    }

    /// Handle a key press while the command palette is open.
    /// Returns true if the key was handled.
    pub(crate) fn handle_command_palette_key(&mut self, key: Key) -> bool {
        if !self.command_palette_is_engaged() {
            return false;
        }

        if let Some(action) = self.keybinding_capture_action {
            return self.handle_keybinding_capture_key(action, key);
        }

        match key {
            Key::Ctrl => {
                self.handle_modifier_key_press(key);
                true
            }
            Key::Escape => {
                self.command_palette_open = false;
                self.clear_command_palette_repeat();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Return => {
                if let Some(command) = self.selected_command() {
                    self.command_palette_open = false;
                    self.clear_command_palette_repeat();
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.record_command_palette_action(command.action);
                    self.handle_action(command.action);
                }
                true
            }
            Key::Up => {
                self.start_command_palette_repeat(Key::Up);
                self.move_command_palette_selection(Key::Up);
                true
            }
            Key::Down => {
                self.start_command_palette_repeat(Key::Down);
                self.move_command_palette_selection(Key::Down);
                true
            }
            Key::Home => {
                self.clear_command_palette_repeat();
                if self.command_palette_selected != 0 || self.command_palette_scroll != 0 {
                    self.command_palette_selected = 0;
                    self.command_palette_scroll = 0;
                    self.needs_redraw = true;
                }
                true
            }
            Key::End => {
                self.clear_command_palette_repeat();
                let filtered = self.filtered_commands();
                if let Some(last_index) = filtered.len().checked_sub(1)
                    && self.command_palette_selected != last_index
                {
                    self.command_palette_selected = last_index;
                    self.command_palette_scroll = filtered
                        .len()
                        .saturating_sub(layout::COMMAND_PALETTE_MAX_VISIBLE);
                    self.needs_redraw = true;
                }
                true
            }
            Key::Backspace if self.modifiers.ctrl => {
                self.delete_previous_command_palette_word();
                true
            }
            Key::Backspace => {
                if !self.command_palette_query.is_empty() {
                    self.command_palette_query.pop();
                    self.mark_command_palette_query_changed();
                }
                true
            }
            Key::Char('u' | 'U') if self.modifiers.ctrl => {
                self.clear_command_palette_query();
                true
            }
            Key::Char('e' | 'E') if self.modifiers.ctrl => {
                if let Some(command) = self.selected_command() {
                    self.begin_keybinding_capture(command.action);
                }
                true
            }
            Key::Delete if self.modifiers.ctrl => {
                if let Some(command) = self.selected_command() {
                    self.request_keybinding_edit(command.action, KeybindingEditOperation::Delete);
                }
                true
            }
            Key::Char('r' | 'R') if self.modifiers.ctrl => {
                if let Some(command) = self.selected_command() {
                    self.request_keybinding_edit(command.action, KeybindingEditOperation::Reset);
                }
                true
            }
            Key::Char(ch) if !self.modifiers.ctrl && !ch.is_control() => {
                self.command_palette_query.push(ch);
                self.mark_command_palette_query_changed();
                true
            }
            Key::Space if !self.modifiers.ctrl => {
                self.command_palette_query.push(' ');
                self.mark_command_palette_query_changed();
                true
            }
            _ => true, // Consume all other keys while palette is open
        }
    }

    fn handle_keybinding_capture_key(&mut self, action: Action, key: Key) -> bool {
        if self.handle_modifier_key_press(key) {
            self.needs_redraw = true;
            return true;
        }
        if matches!(key, Key::Escape) {
            self.keybinding_capture_action = None;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }
        let Some(mut key_label) = key_to_action_label(key) else {
            return true;
        };
        if key_label.len() == 1 && key_label.as_bytes()[0].is_ascii_alphabetic() {
            key_label.make_ascii_uppercase();
        }
        let binding = KeyBinding {
            key: key_label,
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
        }
        .to_string();
        self.keybinding_capture_action = None;
        self.request_keybinding_edit(action, KeybindingEditOperation::Replace(vec![binding]));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    fn start_command_palette_repeat(&mut self, key: Key) {
        self.command_palette_repeat_key = Some(key);
        self.command_palette_repeat_next_tick =
            Some(Instant::now() + COMMAND_PALETTE_REPEAT_INITIAL_DELAY);
    }

    pub(crate) fn clear_command_palette_repeat(&mut self) {
        self.command_palette_repeat_key = None;
        self.command_palette_repeat_next_tick = None;
    }

    fn move_command_palette_selection(&mut self, key: Key) -> bool {
        match key {
            Key::Up => {
                if self.command_palette_selected == 0 {
                    return false;
                }
                self.command_palette_selected -= 1;
                if self.command_palette_selected < self.command_palette_scroll {
                    self.command_palette_scroll = self.command_palette_selected;
                }
            }
            Key::Down => {
                let filtered = self.filtered_commands();
                if self.command_palette_selected + 1 >= filtered.len() {
                    return false;
                }
                self.command_palette_selected += 1;
                if self.command_palette_selected
                    >= self.command_palette_scroll + layout::COMMAND_PALETTE_MAX_VISIBLE
                {
                    self.command_palette_scroll =
                        self.command_palette_selected - layout::COMMAND_PALETTE_MAX_VISIBLE + 1;
                }
            }
            _ => return false,
        }
        self.needs_redraw = true;
        true
    }

    pub(crate) fn release_command_palette_repeat_key(&mut self, key: Key) {
        if self.command_palette_repeat_key == Some(key) {
            self.clear_command_palette_repeat();
        }
    }

    #[allow(dead_code)] // Used by the binary Wayland backend; the lib target has no backend modules.
    pub(crate) fn command_palette_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.command_palette_open {
            return None;
        }
        self.command_palette_repeat_next_tick
            .map(|next_tick| next_tick.saturating_duration_since(now))
    }

    #[allow(dead_code)] // Used by the binary Wayland backend; the lib target has no backend modules.
    pub(crate) fn tick_command_palette_repeat(&mut self, now: Instant) -> bool {
        if !self.command_palette_open {
            self.clear_command_palette_repeat();
            return false;
        }
        let Some(key) = self.command_palette_repeat_key else {
            return false;
        };
        let Some(next_tick) = self.command_palette_repeat_next_tick else {
            return false;
        };
        if now < next_tick {
            return false;
        }

        let changed = self.move_command_palette_selection(key);
        self.command_palette_repeat_next_tick = Some(now + COMMAND_PALETTE_REPEAT_INTERVAL);
        changed
    }

    fn mark_command_palette_query_changed(&mut self) {
        self.command_palette_selected = 0;
        self.command_palette_scroll = 0;
        self.needs_redraw = true;
    }

    fn clear_command_palette_query(&mut self) {
        if self.command_palette_query.is_empty() {
            return;
        }
        self.command_palette_query.clear();
        self.mark_command_palette_query_changed();
    }

    fn delete_previous_command_palette_word(&mut self) {
        if self.command_palette_query.is_empty() {
            return;
        }

        while self
            .command_palette_query
            .chars()
            .last()
            .is_some_and(command_palette_token_separator)
        {
            self.command_palette_query.pop();
        }
        while self
            .command_palette_query
            .chars()
            .last()
            .is_some_and(|ch| !command_palette_token_separator(ch))
        {
            self.command_palette_query.pop();
        }

        self.mark_command_palette_query_changed();
    }

    /// Handle a mouse click while the command palette is open.
    /// Returns true if the click was handled (either on an item or to close the palette).
    pub fn handle_command_palette_click(
        &mut self,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> bool {
        if !self.command_palette_is_engaged() {
            return false;
        }
        if self.keybinding_capture_action.take().is_some() {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }

        let filtered = self.filtered_commands();
        let geometry = self.command_palette_geometry(screen_width, screen_height, filtered.len());
        let (local_x, local_y) = geometry.local_point(x, y);

        // Check if click is outside palette bounds - close it.
        if !geometry.contains_local(local_x, local_y) {
            self.command_palette_open = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }

        // Check command items region.
        if let Some(visible_index) = geometry.visible_item_at(local_x, local_y) {
            // Clicked on item at visible index, actual index accounts for scroll.
            let actual_index = self.command_palette_scroll + visible_index;
            self.command_palette_selected = actual_index;

            if let Some((_, row_action)) = geometry.row_action_at(local_x, local_y)
                && let Some(command) = filtered.get(actual_index).copied()
                && KeybindingsConfig::default()
                    .bindings_for_action(command.action)
                    .is_some()
            {
                match row_action {
                    CommandPaletteRowAction::Edit => {
                        self.begin_keybinding_capture(command.action);
                    }
                    CommandPaletteRowAction::Delete => {
                        self.request_keybinding_edit(
                            command.action,
                            KeybindingEditOperation::Delete,
                        );
                    }
                    CommandPaletteRowAction::Reset => {
                        self.request_keybinding_edit(
                            command.action,
                            KeybindingEditOperation::Reset,
                        );
                    }
                }
                self.needs_redraw = true;
                return true;
            }

            // Get the command label for feedback before executing.
            let label = filtered
                .get(actual_index)
                .map_or("Command", |command| command.label);

            // Execute the command.
            if let Some(command) = filtered.get(actual_index).copied() {
                self.command_palette_open = false;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.record_command_palette_action(command.action);

                // Show brief toast feedback.
                self.set_ui_toast_with_duration(
                    UiToastKind::Info,
                    label,
                    self.command_palette_toast_duration_ms,
                );

                self.handle_action(command.action);
            }
            return true;
        }

        // Click was inside palette but not on an item (e.g., on input field or padding).
        true
    }

    /// Determine the cursor type for a given point within the command palette.
    /// Returns `None` if the command palette is not open or the point is outside.
    pub fn command_palette_cursor_hint_at(
        &self,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<CommandPaletteCursorHint> {
        if !self.command_palette_open {
            return None;
        }

        let filtered = self.filtered_commands();
        let geometry = self.command_palette_geometry(screen_width, screen_height, filtered.len());
        command_palette_cursor_hint_from_local(geometry, x, y)
    }

    pub(crate) fn command_palette_action_tooltip(
        &self,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<(&'static str, i32, i32)> {
        if !self.command_palette_open {
            return None;
        }
        let filtered = self.filtered_commands();
        let geometry = self.command_palette_geometry(screen_width, screen_height, filtered.len());
        let (x, y) = self.pointer_position();
        let (local_x, local_y) = geometry.local_point(x, y);
        let (visible_index, action) = geometry.row_action_at(local_x, local_y)?;
        let actual_index = self.command_palette_scroll + visible_index;
        let command = filtered.get(actual_index)?;
        KeybindingsConfig::default().bindings_for_action(command.action)?;
        Some((action.tooltip(), x, y))
    }
}

fn command_palette_token_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '+' || ch == '/'
}

fn command_palette_cursor_hint_from_local(
    geometry: CommandPaletteGeometry,
    x: i32,
    y: i32,
) -> Option<CommandPaletteCursorHint> {
    let (local_x, local_y) = geometry.local_point(x, y);

    // Check if outside palette bounds.
    if !geometry.contains_local(local_x, local_y) {
        return None;
    }

    // Check input field region.
    if geometry.local_in_input(local_x, local_y) {
        return Some(CommandPaletteCursorHint::Text);
    }

    // Check command items region.
    if geometry.visible_item_at(local_x, local_y).is_some() {
        return Some(CommandPaletteCursorHint::Pointer);
    }

    Some(CommandPaletteCursorHint::Default)
}
