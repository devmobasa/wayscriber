use super::super::base::{
    InputState, KeybindingEditOperation, KeybindingEditRequest, Toast, ToastPriority,
};
use super::search::CommandPaletteListRow;
use super::{
    CommandPaletteCursorHint,
    layout::{CommandPaletteGeometry, CommandPaletteRowAction},
};
use crate::config::{KeyBinding, action_label, keybindings::default_keybindings};
use crate::configurator_destination::keybindings_destination_for_action;
use crate::domain::Action;
use crate::input::events::Key;
use crate::input::state::actions::key_press::bindings::key_to_action_label;
use std::time::{Duration, Instant};

impl InputState {
    pub fn keybinding_capture_action(&self) -> Option<Action> {
        self.keymap.capture_action()
    }

    /// Whether the palette owns keyboard and pointer input.
    ///
    /// This is what the toolbar and GTK modal gates ask, which is why it keeps
    /// its own name rather than reading the visibility flag directly: the
    /// shortcut-capture modal is its second half, and it must swallow the same
    /// keys and pointer events the list does.
    pub(crate) fn command_palette_is_engaged(&self) -> bool {
        self.command_palette.open || self.keymap.capture_action().is_some()
    }

    /// Arm the capture modal for one action's next keyboard chord.
    ///
    /// The chord it reads is durable: the backend writes that one action's
    /// `[keybindings]` entry to `config.toml` before installing the new keymap,
    /// and a save that fails degrades to a this-run edit whose toast says so.
    /// An action with no `[keybindings]` field has nothing to rebind, so the
    /// affordance explains instead of opening a modal that cannot succeed.
    pub fn begin_keybinding_capture(&mut self, action: Action) -> bool {
        if default_keybindings().bindings_for_action(action).is_none() {
            self.push_toast(
                ToastPriority::Info,
                "palette.shortcut",
                Toast::warning(format!(
                    "{} has no configurable keyboard shortcut.",
                    action_label(action)
                )),
            );
            return false;
        }
        self.keymap.begin_capture(action);
        self.clear_command_palette_repeat();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Queue a shortcut edit for the backend, which owns the keymap.
    ///
    /// Queued behind whatever is already waiting: the backend drains the whole
    /// list on its next pass, so recording a second edit before it gets there
    /// costs the first neither its write nor its toast.
    pub(crate) fn request_keybinding_edit(
        &mut self,
        action: Action,
        operation: KeybindingEditOperation,
    ) -> bool {
        if default_keybindings().bindings_for_action(action).is_none() {
            self.push_toast(
                ToastPriority::Info,
                "palette.shortcut",
                Toast::warning(format!(
                    "{} has no configurable keyboard shortcut.",
                    action_label(action)
                )),
            );
            return false;
        }
        self.emit_input_effect(super::super::base::InputEffect::KeybindingEdit(
            KeybindingEditRequest { action, operation },
        ));
        true
    }

    /// Hand one action's shortcut to the configurator.
    ///
    /// The palette's own controls rebind one chord in place; this is the route
    /// to the full editor, opening the Keybindings screen on the section that
    /// holds this action with the action's own key searched. Launching closes
    /// the overlay, so the palette closes with it.
    pub(crate) fn open_configurator_for_shortcut(&mut self, action: Action) -> bool {
        let Some(destination) = keybindings_destination_for_action(action) else {
            self.push_toast(
                ToastPriority::Info,
                "palette.shortcut",
                Toast::warning(format!(
                    "{} has no configurable keyboard shortcut.",
                    action_label(action)
                )),
            );
            return false;
        };
        self.command_palette.close();
        self.clear_command_palette_repeat();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.launch_configurator(Some(destination));
        true
    }

    fn open_command_palette_internal(&mut self, track_usage: bool) {
        self.close_modals_for_open(crate::input::state::core::modal::ModalSurface::CommandPalette);
        self.command_palette.open();
        self.clear_command_palette_repeat();
        if track_usage {
            self.pending_onboarding_usage.used_command_palette = true;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Toggle the command palette visibility.
    pub(crate) fn toggle_command_palette(&mut self) {
        if self.command_palette.open {
            self.command_palette.close();
            self.clear_command_palette_repeat();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }
        self.open_command_palette_internal(true);
    }

    /// Handle a key press while the command palette is open.
    /// Returns true if the key was handled.
    pub(crate) fn handle_command_palette_key_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        key: Key,
    ) -> bool {
        if !self.command_palette_is_engaged() {
            return false;
        }

        if let Some(action) = self.keymap.capture_action() {
            return self.handle_keybinding_capture_key(action, key);
        }

        match key {
            // Every modifier a `KeyBinding` can carry is tracked here, because
            // the shortcut controls and the capture modal read all four:
            // without this arm the palette would swallow the press, Ctrl+Shift+E
            // could never be told apart from Ctrl+E, and a Super already held
            // when capture began would be missing from the chord that gets
            // saved. `on_key_release` clears them again on the way out,
            // whatever is open at the time. `Key::Tab` stays with the catch-all
            // below: it is the pointer-drag modifier, not part of a chord.
            Key::Ctrl | Key::Shift | Key::Alt | Key::Super => {
                self.handle_modifier_key_press(key);
                true
            }
            Key::Escape => {
                self.command_palette.close();
                self.clear_command_palette_repeat();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Return => {
                if let Some(command) = self.selected_command() {
                    self.command_palette.close();
                    self.clear_command_palette_repeat();
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.record_command_palette_action(command.action);
                    self.handle_action_with_resources(resources, command.action);
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
            Key::Home | Key::End => {
                self.clear_command_palette_repeat();
                self.move_command_palette_selection(key);
                true
            }
            Key::Backspace if self.modifiers.ctrl => {
                self.needs_redraw |= self.command_palette.delete_previous_word();
                true
            }
            Key::Backspace => {
                self.needs_redraw |= self.command_palette.backspace();
                true
            }
            Key::Char('u' | 'U') if self.modifiers.ctrl => {
                self.needs_redraw |= self.command_palette.set_query("");
                true
            }
            // Shift is checked first: the configurator route and the in-place
            // capture share the letter, and the plain-Ctrl arm below matches
            // both cases.
            Key::Char('e' | 'E') if self.modifiers.ctrl && self.modifiers.shift => {
                if let Some(command) = self.selected_command() {
                    self.open_configurator_for_shortcut(command.action);
                }
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
                self.command_palette.append(ch);
                self.needs_redraw = true;
                true
            }
            Key::Space if !self.modifiers.ctrl => {
                self.command_palette.append(' ');
                self.needs_redraw = true;
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
            self.keymap.clear_capture();
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
            logo: self.modifiers.logo,
        }
        .to_string();
        self.keymap.clear_capture();
        self.request_keybinding_edit(action, KeybindingEditOperation::Replace(vec![binding]));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    fn start_command_palette_repeat(&mut self, key: Key) {
        self.command_palette.start_repeat(key, Instant::now());
    }
    pub(crate) fn clear_command_palette_repeat(&mut self) {
        self.command_palette.clear_repeat();
    }
    pub(crate) fn reconcile_command_palette_scroll(&mut self) {
        let rows = self.command_palette_rows();
        let capacity = self.command_palette_row_capacity();
        self.command_palette.reconcile_scroll(&rows, capacity);
    }
    fn move_command_palette_selection(&mut self, key: Key) -> bool {
        let rows = self.command_palette_rows();
        let capacity = self.command_palette_row_capacity();
        let changed = self.command_palette.navigate(key, &rows, capacity);
        self.needs_redraw |= changed;
        changed
    }
    pub fn command_palette_wheel_scroll(&mut self, direction: i32) {
        let rows = self.command_palette_rows();
        let capacity = self.command_palette_row_capacity();
        self.needs_redraw |= self
            .command_palette
            .wheel_scroll(direction, &rows, capacity);
    }
    pub(crate) fn release_command_palette_repeat_key(&mut self, key: Key) {
        self.command_palette.release_repeat(key);
    }
    pub(crate) fn command_palette_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        self.command_palette.repeat_timeout(now)
    }
    pub(crate) fn tick_command_palette_repeat(&mut self, now: Instant) -> bool {
        let rows = self.command_palette_rows();
        let capacity = self.command_palette_row_capacity();
        let changed = self.command_palette.tick_repeat(now, &rows, capacity);
        self.needs_redraw |= changed;
        changed
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
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        self.handle_command_palette_click_with_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            x,
            y,
            screen_width,
            screen_height,
        )
    }

    pub(crate) fn handle_command_palette_click_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> bool {
        if !self.command_palette_is_engaged() {
            return false;
        }
        if self.keymap.take_capture().is_some() {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }

        let rows = self.command_palette_rows();
        let geometry = self.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
        let (local_x, local_y) = geometry.local_point(x, y);

        // Check if click is outside palette bounds - close it.
        if !geometry.contains_local(local_x, local_y) {
            self.command_palette.close();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }

        // Check command items region.
        if let Some(visible_index) = geometry.visible_item_at(local_x, local_y) {
            // Clicked on row at visible index, actual index accounts for scroll.
            let display_index = self.command_palette.scroll + visible_index;
            let Some(command_entry) = rows.get(display_index).and_then(|row| match row {
                CommandPaletteListRow::Header(_) => None,
                CommandPaletteListRow::Command {
                    command,
                    command_index,
                } => Some((*command, *command_index)),
            }) else {
                // Group headers consume the click without acting.
                return true;
            };
            let (command, actual_index) = command_entry;
            self.command_palette.select_command(actual_index);

            if let Some((_, row_action)) = geometry.row_action_at(local_x, local_y)
                && default_keybindings()
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

            // Execute the command.
            self.command_palette.close();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.record_command_palette_action(command.action);

            // Show brief toast feedback.
            self.push_toast(
                ToastPriority::Info,
                "palette.feedback",
                Toast::info(command.label).duration_ms(self.command_palette_toast_duration_ms()),
            );

            self.handle_action_with_resources(resources, command.action);
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
        if !self.command_palette.open {
            return None;
        }

        let rows = self.command_palette_rows();
        let geometry = self.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
        command_palette_cursor_hint_from_local(geometry, &rows, self.command_palette.scroll, x, y)
    }

    #[cfg(test)]
    pub(crate) fn command_palette_action_tooltip(
        &self,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<(&'static str, i32, i32)> {
        if !self.command_palette.open {
            return None;
        }
        let rows = self.command_palette_rows();
        let geometry = self.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
        self.command_palette_action_tooltip_for_layout(&rows, geometry)
    }

    pub(crate) fn command_palette_action_tooltip_for_layout(
        &self,
        rows: &[CommandPaletteListRow],
        geometry: CommandPaletteGeometry,
    ) -> Option<(&'static str, i32, i32)> {
        if !self.command_palette.open {
            return None;
        }
        let (x, y) = self.pointer_position();
        let (local_x, local_y) = geometry.local_point(x, y);
        let (visible_index, action) = geometry.row_action_at(local_x, local_y)?;
        let display_index = self.command_palette.scroll + visible_index;
        let command = match rows.get(display_index)? {
            CommandPaletteListRow::Header(_) => return None,
            CommandPaletteListRow::Command { command, .. } => command,
        };
        default_keybindings().bindings_for_action(command.action)?;
        Some((action.tooltip(), x, y))
    }
}

fn command_palette_cursor_hint_from_local(
    geometry: CommandPaletteGeometry,
    rows: &[CommandPaletteListRow],
    scroll: usize,
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

    // Group headers are not interactive.
    if let Some(visible_index) = geometry.visible_item_at(local_x, local_y)
        && matches!(
            rows.get(scroll + visible_index),
            Some(CommandPaletteListRow::Header(_))
        )
    {
        return Some(CommandPaletteCursorHint::Default);
    }

    // Check command items region.
    if geometry.visible_item_at(local_x, local_y).is_some() {
        return Some(CommandPaletteCursorHint::Pointer);
    }

    Some(CommandPaletteCursorHint::Default)
}
