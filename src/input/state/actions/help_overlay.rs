use crate::input::events::Key;

use super::super::InputState;

impl InputState {
    pub(in crate::input::state) fn handle_help_overlay_key(&mut self, key: Key) -> bool {
        if !self.help_overlay.visible {
            return false;
        }

        let search_active = !self.help_overlay.search.trim().is_empty();

        match key {
            Key::Escape => {
                // Escape clears search first, then closes overlay
                if search_active {
                    self.help_overlay.search.clear();
                    self.help_overlay.scroll = 0.0;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                } else {
                    self.toggle_help_overlay();
                }
                true
            }
            Key::F1 | Key::F10 => {
                self.toggle_help_overlay();
                true
            }
            Key::Backspace if !self.help_overlay.search.is_empty() => {
                self.help_overlay.search.pop();
                self.help_overlay.scroll = 0.0;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Backspace => false,
            Key::Space => {
                if search_active {
                    self.help_overlay.search.push(' ');
                    self.help_overlay.scroll = 0.0;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
                true
            }
            Key::Char(ch) if !ch.is_control() => {
                self.help_overlay.search.push(ch);
                self.help_overlay.scroll = 0.0;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Char(_) => false,
            // Disable page navigation while search is active
            Key::Left | Key::Right | Key::PageUp | Key::PageDown | Key::Home | Key::End
                if search_active =>
            {
                true
            }
            Key::Left | Key::PageUp if !search_active => self.help_overlay_prev_page(),
            Key::Right | Key::PageDown if !search_active => self.help_overlay_next_page(),
            Key::Home if !search_active && self.help_overlay.page != 0 => {
                self.help_overlay.page = 0;
                self.help_overlay.scroll = 0.0;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Home if !search_active => false,
            Key::End if !search_active => {
                // Use the max page constant; actual page count is computed during render
                // and the page index will be clamped appropriately.
                let last_page = 9; // HELP_OVERLAY_MAX_PAGES - 1
                if self.help_overlay.page != last_page {
                    self.help_overlay.page = last_page;
                    self.help_overlay.scroll = 0.0;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
