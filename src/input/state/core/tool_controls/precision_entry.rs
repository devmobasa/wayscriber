//! Precise numeric entry popup state (opened from the style pill's
//! numeral buttons).
//!
//! The popup is the second Cairo keyboard surface after the color popup's
//! hex field: digits/backspace edit the buffer, Enter commits (the
//! toolbar apply arm clamps to the slider range), Esc cancels. Any other
//! toolbar interaction dismisses it (see the backend dismissal rules).

use super::super::base::InputState;
use crate::input::events::Key;
use crate::ui::toolbar::PrecisionEntryTarget;

/// Live state of the precise-entry popup.
#[derive(Debug, Clone, PartialEq)]
pub struct PrecisionEntryState {
    pub target: PrecisionEntryTarget,
    /// Typed buffer (digits and at most one decimal point).
    pub buffer: String,
    /// Replace-on-type: the prefilled value is selected when the popup
    /// opens, mirroring the hex field's behavior.
    pub selected: bool,
}

impl InputState {
    pub fn is_precision_entry_open(&self) -> bool {
        self.precision_entry.is_some()
    }

    pub fn precision_entry(&self) -> Option<&PrecisionEntryState> {
        self.precision_entry.as_ref()
    }

    /// Open the precise-entry popup prefilled with the target's current
    /// value (selected, so the first keystroke replaces it).
    pub fn open_precision_entry(&mut self, target: PrecisionEntryTarget) {
        self.close_radial_menu();
        let value = match target {
            PrecisionEntryTarget::Thickness => {
                if self.active_tool().uses_eraser_size()
                    || self
                        .tool_override()
                        .is_some_and(|tool| tool.uses_eraser_size())
                {
                    self.eraser_size
                } else {
                    self.current_thickness
                }
            }
            PrecisionEntryTarget::FontSize => self.current_font_size,
        };
        self.precision_entry = Some(PrecisionEntryState {
            target,
            buffer: format!("{value:.0}"),
            selected: true,
        });
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Close the popup without applying.
    pub fn cancel_precision_entry(&mut self) -> bool {
        if self.precision_entry.take().is_none() {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Parse the buffer and close the popup; the caller (the toolbar
    /// apply arm) clamps and applies the returned value.
    pub fn take_precision_entry_commit(&mut self) -> Option<(PrecisionEntryTarget, f64)> {
        let state = self.precision_entry.take()?;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        let value = state.buffer.parse::<f64>().ok()?;
        value.is_finite().then_some((state.target, value))
    }

    fn precision_entry_append(&mut self, ch: char) {
        let Some(state) = self.precision_entry.as_mut() else {
            return;
        };
        if state.selected {
            state.buffer.clear();
            state.selected = false;
        }
        let valid = ch.is_ascii_digit() || (ch == '.' && !state.buffer.contains('.'));
        if valid && state.buffer.len() < 6 {
            state.buffer.push(ch);
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    fn precision_entry_backspace(&mut self) {
        let Some(state) = self.precision_entry.as_mut() else {
            return;
        };
        if state.selected {
            state.buffer.clear();
            state.selected = false;
        } else {
            state.buffer.pop();
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Keyboard handling while the popup is open (the same shape as
    /// `handle_color_picker_popup_key`): every key is consumed.
    pub(in crate::input::state) fn handle_precision_entry_key(&mut self, key: Key) -> bool {
        if !self.is_precision_entry_open() {
            return false;
        }
        match key {
            Key::Escape => {
                self.cancel_precision_entry();
            }
            Key::Return => {
                if let Some((target, value)) = self.take_precision_entry_commit() {
                    let event =
                        crate::ui::toolbar::ToolbarEvent::CommitPrecisionEntry { target, value };
                    let _ = self.apply_toolbar_event(event);
                }
            }
            Key::Backspace | Key::Delete => self.precision_entry_backspace(),
            Key::Char(ch) => self.precision_entry_append(ch),
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn open_prefills_the_selected_current_value_and_typing_replaces_it() {
        let mut state = make_test_input_state();
        state.current_thickness = 4.0;
        assert!(state.apply_toolbar_event(ToolbarEvent::OpenPrecisionEntry(
            PrecisionEntryTarget::Thickness
        )));
        let entry = state.precision_entry().expect("open entry");
        assert_eq!(entry.target, PrecisionEntryTarget::Thickness);
        assert_eq!(entry.buffer, "4");
        assert!(entry.selected, "prefill is selected (replace-on-type)");

        // First digit replaces the selection; further digits append; only
        // digits and one decimal point are accepted.
        assert!(state.handle_precision_entry_key(Key::Char('1')));
        assert!(state.handle_precision_entry_key(Key::Char('2')));
        assert!(state.handle_precision_entry_key(Key::Char('.')));
        assert!(state.handle_precision_entry_key(Key::Char('.')));
        assert!(state.handle_precision_entry_key(Key::Char('x')));
        assert!(state.handle_precision_entry_key(Key::Char('5')));
        let entry = state.precision_entry().expect("open entry");
        assert_eq!(entry.buffer, "12.5");
        assert!(!entry.selected);

        assert!(state.handle_precision_entry_key(Key::Backspace));
        assert_eq!(state.precision_entry().expect("entry").buffer, "12.");
    }

    #[test]
    fn enter_commits_the_clamped_value_and_esc_cancels() {
        let mut state = make_test_input_state();
        state.current_thickness = 4.0;
        state.open_precision_entry(PrecisionEntryTarget::Thickness);
        for ch in "999".chars() {
            let _ = state.handle_precision_entry_key(Key::Char(ch));
        }
        assert!(state.handle_precision_entry_key(Key::Return));
        assert!(!state.is_precision_entry_open());
        // Clamped to the shared thickness slider range.
        assert_eq!(
            state.current_thickness,
            crate::ui::toolbar::model::ToolbarSliderSpec::THICKNESS.max
        );

        // Esc restores nothing and applies nothing.
        let before = state.current_thickness;
        state.open_precision_entry(PrecisionEntryTarget::Thickness);
        let _ = state.handle_precision_entry_key(Key::Char('7'));
        assert!(state.handle_precision_entry_key(Key::Escape));
        assert!(!state.is_precision_entry_open());
        assert_eq!(state.current_thickness, before);

        // A closed popup consumes no keys.
        assert!(!state.handle_precision_entry_key(Key::Char('1')));
    }

    #[test]
    fn font_size_target_commits_through_the_font_apply_arm() {
        let mut state = make_test_input_state();
        state.open_precision_entry(PrecisionEntryTarget::FontSize);
        assert_eq!(
            state.precision_entry().expect("entry").buffer,
            format!("{:.0}", state.current_font_size)
        );
        assert!(
            state.apply_toolbar_event(ToolbarEvent::CommitPrecisionEntry {
                target: PrecisionEntryTarget::FontSize,
                value: 1000.0,
            })
        );
        assert!(!state.is_precision_entry_open());
        assert_eq!(
            state.current_font_size,
            crate::ui::toolbar::model::ToolbarSliderSpec::FONT_SIZE.max
        );

        // An unparseable buffer commits nothing.
        state.open_precision_entry(PrecisionEntryTarget::FontSize);
        let _ = state.handle_precision_entry_key(Key::Backspace);
        assert!(state.take_precision_entry_commit().is_none());
    }

    #[test]
    fn overlay_pointer_press_cancels_the_popup() {
        let mut state = make_test_input_state();
        state.open_precision_entry(PrecisionEntryTarget::Thickness);
        state.on_mouse_press(crate::input::MouseButton::Left, 100, 100);
        assert!(!state.is_precision_entry_open());
    }

    #[test]
    fn opening_precision_entry_closes_radial_menu() {
        let mut state = make_test_input_state();
        state.open_radial_menu(320.0, 240.0);
        assert!(state.is_radial_menu_open());

        state.open_precision_entry(PrecisionEntryTarget::Thickness);

        assert!(state.is_precision_entry_open());
        assert!(!state.is_radial_menu_open());

        state.open_radial_menu(320.0, 240.0);
        assert!(state.is_radial_menu_open());
        assert!(!state.is_precision_entry_open());
    }
}
