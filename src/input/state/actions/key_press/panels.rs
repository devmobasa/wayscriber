use crate::input::events::Key;
use crate::input::state::InputState;

const PROPERTIES_PANEL_COARSE_STEP: i32 = 5;

impl InputState {
    pub(super) fn handle_properties_panel_key(&mut self, key: Key) -> bool {
        let adjust_step = if self.modifiers.shift {
            PROPERTIES_PANEL_COARSE_STEP
        } else {
            1
        };
        match key {
            Key::Escape => {
                self.close_properties_panel();
                true
            }
            Key::Up => self.focus_previous_properties_entry(),
            Key::Down => self.focus_next_properties_entry(),
            Key::Home => self.focus_first_properties_entry(),
            Key::End => self.focus_last_properties_entry(),
            Key::Return | Key::Space => self.activate_properties_panel_entry(),
            Key::Left => self.adjust_properties_panel_entry(-adjust_step),
            Key::Right => self.adjust_properties_panel_entry(adjust_step),
            Key::Char('+') | Key::Char('=') => self.adjust_properties_panel_entry(adjust_step),
            Key::Char('-') | Key::Char('_') => self.adjust_properties_panel_entry(-adjust_step),
            _ => false,
        }
    }

    pub(super) fn handle_context_menu_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.close_context_menu();
                true
            }
            Key::Up => self.focus_previous_context_menu_entry(),
            Key::Down => self.focus_next_context_menu_entry(),
            Key::Home => self.focus_first_context_menu_entry(),
            Key::End => self.focus_last_context_menu_entry(),
            Key::Return | Key::Space => self.activate_context_menu_selection(),
            _ => false,
        }
    }
}
