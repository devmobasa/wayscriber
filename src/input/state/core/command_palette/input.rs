use super::super::base::{InputState, UiToastKind};
use super::layout;
use super::{CommandPaletteCursorHint, layout::CommandPaletteGeometry};
use crate::input::events::Key;

impl InputState {
    fn open_command_palette_internal(&mut self, track_usage: bool) {
        self.command_palette_open = true;
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
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }
        self.open_command_palette_internal(true);
    }

    /// Handle a key press while the command palette is open.
    /// Returns true if the key was handled.
    pub(crate) fn handle_command_palette_key(&mut self, key: Key) -> bool {
        if !self.command_palette_open {
            return false;
        }

        match key {
            Key::Escape => {
                self.command_palette_open = false;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Return => {
                if let Some(command) = self.selected_command() {
                    self.command_palette_open = false;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.record_command_palette_action(command.action);
                    self.handle_action(command.action);
                }
                true
            }
            Key::Up => {
                if self.command_palette_selected > 0 {
                    self.command_palette_selected -= 1;
                    // Adjust scroll if selection moves above visible window
                    if self.command_palette_selected < self.command_palette_scroll {
                        self.command_palette_scroll = self.command_palette_selected;
                    }
                    self.needs_redraw = true;
                }
                true
            }
            Key::Down => {
                let filtered = self.filtered_commands();
                if self.command_palette_selected + 1 < filtered.len() {
                    self.command_palette_selected += 1;
                    // Adjust scroll if selection moves below visible window
                    if self.command_palette_selected
                        >= self.command_palette_scroll + layout::COMMAND_PALETTE_MAX_VISIBLE
                    {
                        self.command_palette_scroll =
                            self.command_palette_selected - layout::COMMAND_PALETTE_MAX_VISIBLE + 1;
                    }
                    self.needs_redraw = true;
                }
                true
            }
            Key::Backspace => {
                if !self.command_palette_query.is_empty() {
                    self.command_palette_query.pop();
                    self.command_palette_selected = 0;
                    self.command_palette_scroll = 0;
                    self.needs_redraw = true;
                }
                true
            }
            Key::Char(ch) if !ch.is_control() => {
                self.command_palette_query.push(ch);
                self.command_palette_selected = 0;
                self.command_palette_scroll = 0;
                self.needs_redraw = true;
                true
            }
            Key::Space => {
                self.command_palette_query.push(' ');
                self.command_palette_selected = 0;
                self.command_palette_scroll = 0;
                self.needs_redraw = true;
                true
            }
            _ => true, // Consume all other keys while palette is open
        }
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
        if !self.command_palette_open {
            return false;
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
