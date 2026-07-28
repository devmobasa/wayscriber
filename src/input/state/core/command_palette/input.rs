use super::super::base::{InputState, Toast, ToastPriority};
use super::layout;
use super::search::{CommandPaletteListRow, command_palette_display_index};
use super::{CommandPaletteCursorHint, layout::CommandPaletteGeometry};
use crate::config::action_label;
use crate::configurator_destination::keybindings_destination_for_action;
use crate::domain::Action;
use crate::input::events::Key;
use std::time::{Duration, Instant};

const COMMAND_PALETTE_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(280);
const COMMAND_PALETTE_REPEAT_INTERVAL: Duration = Duration::from_millis(55);

impl InputState {
    /// Whether the palette owns keyboard and pointer input.
    ///
    /// This is what the toolbar and GTK modal gates ask, which is why it keeps
    /// its own name rather than reading the visibility flag directly. The
    /// shortcut-capture modal used to be its second half; the palette no longer
    /// captures chords, so the open flag is all that is left of it.
    pub(crate) fn command_palette_is_engaged(&self) -> bool {
        self.command_palette_open
    }

    /// Hand one action's shortcut to the configurator.
    ///
    /// The overlay stopped editing `[keybindings]`, so the palette's shortcut
    /// affordance is navigation: it opens the Keybindings screen on the
    /// section that holds this action, with the action's own key searched.
    /// Launching closes the overlay, so the palette closes with it.
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
        self.command_palette_open = false;
        self.clear_command_palette_repeat();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.launch_configurator(Some(destination));
        true
    }

    fn open_command_palette_internal(&mut self, track_usage: bool) {
        self.close_radial_menu();
        self.command_palette_open = true;
        self.clear_command_palette_repeat();
        if track_usage {
            self.pending_onboarding_usage.used_command_palette = true;
        }
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.command_palette_scroll = 0;
        // Close other overlays. Route help through the canonical closer so the
        // cached pointer hit map is dropped; setting `show_help = false` alone
        // would leave the previous layout hittable until the next render.
        if self.show_help {
            self.close_help_overlay();
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
                    // Scroll is in display-row space (headers included).
                    self.command_palette_scroll = self
                        .command_palette_rows()
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
                    self.open_configurator_for_shortcut(command.action);
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
                let rows = self.command_palette_rows();
                let display_index =
                    command_palette_display_index(&rows, self.command_palette_selected);
                // Keep the group header visible when the selection sits
                // directly beneath it.
                let target_top = if display_index > 0
                    && matches!(
                        rows.get(display_index - 1),
                        Some(CommandPaletteListRow::Header(_))
                    ) {
                    display_index - 1
                } else {
                    display_index
                };
                if target_top < self.command_palette_scroll {
                    self.command_palette_scroll = target_top;
                }
            }
            Key::Down => {
                let filtered = self.filtered_commands();
                if self.command_palette_selected + 1 >= filtered.len() {
                    return false;
                }
                self.command_palette_selected += 1;
                let rows = self.command_palette_rows();
                let display_index =
                    command_palette_display_index(&rows, self.command_palette_selected);
                if display_index
                    >= self.command_palette_scroll + layout::COMMAND_PALETTE_MAX_VISIBLE
                {
                    self.command_palette_scroll =
                        display_index - layout::COMMAND_PALETTE_MAX_VISIBLE + 1;
                }
            }
            _ => return false,
        }
        self.needs_redraw = true;
        true
    }

    /// Scroll the palette list by one display row (mouse wheel). Keeps the
    /// selection inside the visible window, skipping header rows.
    pub fn command_palette_wheel_scroll(&mut self, direction: i32) {
        if direction == 0 || !self.command_palette_open {
            return;
        }
        let rows = self.command_palette_rows();
        let max_scroll = rows
            .len()
            .saturating_sub(layout::COMMAND_PALETTE_MAX_VISIBLE);
        if direction > 0 {
            if self.command_palette_scroll >= max_scroll {
                return;
            }
            self.command_palette_scroll += 1;
        } else {
            if self.command_palette_scroll == 0 {
                return;
            }
            self.command_palette_scroll -= 1;
        }

        let window_start = self.command_palette_scroll;
        let window_end = window_start + layout::COMMAND_PALETTE_MAX_VISIBLE;
        let selected_display = command_palette_display_index(&rows, self.command_palette_selected);
        if selected_display < window_start {
            if let Some(command_index) = rows[window_start..window_end.min(rows.len())]
                .iter()
                .find_map(|row| row.command_index())
            {
                self.command_palette_selected = command_index;
            }
        } else if selected_display >= window_end
            && let Some(command_index) = rows[window_start..window_end.min(rows.len())]
                .iter()
                .rev()
                .find_map(|row| row.command_index())
        {
            self.command_palette_selected = command_index;
        }
        self.needs_redraw = true;
    }

    pub(crate) fn release_command_palette_repeat_key(&mut self, key: Key) {
        if self.command_palette_repeat_key == Some(key) {
            self.clear_command_palette_repeat();
        }
    }

    pub(crate) fn command_palette_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.command_palette_open {
            return None;
        }
        self.command_palette_repeat_next_tick
            .map(|next_tick| next_tick.saturating_duration_since(now))
    }

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

        let rows = self.command_palette_rows();
        let geometry = self.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
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
            // Clicked on row at visible index, actual index accounts for scroll.
            let display_index = self.command_palette_scroll + visible_index;
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
            self.command_palette_selected = actual_index;

            // Execute the command.
            self.command_palette_open = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.record_command_palette_action(command.action);

            // Show brief toast feedback.
            self.push_toast(
                ToastPriority::Info,
                "palette.feedback",
                Toast::info(command.label).duration_ms(self.command_palette_toast_duration_ms),
            );

            self.handle_action(command.action);
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

        let rows = self.command_palette_rows();
        let geometry = self.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
        command_palette_cursor_hint_from_local(geometry, &rows, self.command_palette_scroll, x, y)
    }
}

fn command_palette_token_separator(ch: char) -> bool {
    ch.is_whitespace() || ch == '+' || ch == '/'
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
