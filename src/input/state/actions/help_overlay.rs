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
                    self.help_overlay.clear_search_text();
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
                self.help_overlay.pop_search_char();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Backspace => false,
            Key::Space => {
                if search_active {
                    self.help_overlay.push_search_char(' ');
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
                true
            }
            Key::Char(ch) if !ch.is_control() => {
                self.help_overlay.push_search_char(ch);
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
            Key::Home if !search_active => {
                let changed = self.help_overlay.first_page();
                if changed {
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
                changed
            }
            Key::End if !search_active => {
                let changed = self.help_overlay.last_page();
                if changed {
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
                changed
            }
            _ => false,
        }
    }
}
