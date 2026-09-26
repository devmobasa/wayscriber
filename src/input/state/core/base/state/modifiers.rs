use super::super::types::DrawingState;
use super::structs::InputState;
use crate::input::Modifiers;

impl InputState {
    /// Resets all tracked keyboard modifiers to the "released" state.
    ///
    /// This is used as a safety net when external UI (portals, other windows)
    /// or focus transitions may cause us to miss key release events from
    /// the compositor, which would otherwise leave modifiers "stuck" and break
    /// shortcut handling and tool selection.
    pub fn reset_modifiers(&mut self) {
        self.keymap.clear_consumed_pointer_buttons();
        self.clear_pending_sequence();
        self.update_modifiers(|modifiers| *modifiers = Modifiers::new());
    }

    /// Clears key state whose release can be lost with keyboard focus.
    ///
    /// The backend owns its own repeat and board-pan latches; this is the
    /// `InputState` half shared by protocol focus leave and synthetic focus
    /// loss during layer-output recreation.
    pub(crate) fn clear_focus_owned_key_state(&mut self) {
        self.reset_modifiers();
        self.clear_command_palette_repeat();
        self.clear_font_picker_repeat();
    }

    /// Synchronize modifier state from backend-provided values (e.g. compositor).
    ///
    /// This lets us correct cases where a key release event was missed but the compositor's
    /// authoritative modifier state is still accurate. Returns whether any modifier changed.
    pub fn sync_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool, logo: bool) -> bool {
        self.update_modifiers(|modifiers| {
            modifiers.shift = shift;
            modifiers.ctrl = ctrl;
            modifiers.alt = alt;
            modifiers.logo = logo;
            // Tab has no direct compositor flag; leave it unchanged.
        })
    }

    /// Applies one modifier update and refreshes what depends on modifiers.
    /// Returns whether any modifier changed.
    ///
    /// Modifiers pick the drag tool, so a change can switch the tool the
    /// status bar and the tool preview show. A compositor sync or key release
    /// is often the only event in that change, so it requests the redraw
    /// itself. The canvas stays clean: the render damages only that chrome.
    pub(in crate::input::state) fn update_modifiers(
        &mut self,
        update: impl FnOnce(&mut Modifiers),
    ) -> bool {
        let before = self.modifiers;
        update(&mut self.modifiers);

        if matches!(self.state, DrawingState::Idle) {
            self.sync_current_settings_from_active_tool();
        }
        let changed = self.modifiers != before;
        if changed {
            self.needs_redraw = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{Key, Tool};

    #[test]
    fn a_modifier_only_sync_redraws_the_tool_chrome_without_dirtying_the_canvas() {
        let mut state = make_test_input_state();
        state.update_screen_dimensions(800, 600);
        let base_tool = state.active_tool();
        let _ = state.take_dirty_regions();
        state.needs_redraw = false;

        assert!(state.sync_modifiers(false, true, false, false));
        assert!(
            state.needs_redraw,
            "the Ctrl drag tool must reach the status bar"
        );
        assert_eq!(state.active_tool(), Tool::Rect);

        state.needs_redraw = false;
        assert!(state.sync_modifiers(false, false, false, false));
        assert!(
            state.needs_redraw,
            "releasing Ctrl must restore the status bar"
        );
        assert_eq!(state.active_tool(), base_tool);
        assert!(state.take_dirty_regions().is_empty());
    }

    #[test]
    fn an_unchanged_modifier_sync_requests_no_redraw() {
        let mut state = make_test_input_state();
        state.sync_modifiers(true, false, false, false);
        state.needs_redraw = false;

        assert!(!state.sync_modifiers(true, false, false, false));
        assert!(!state.needs_redraw);
    }

    #[test]
    fn a_modifier_key_release_redraws_the_tool_chrome() {
        let mut state = make_test_input_state();
        state.on_key_press(Key::Ctrl);
        state.needs_redraw = false;

        state.on_key_release(Key::Ctrl);

        assert!(!state.modifiers.ctrl);
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        state.on_key_release(Key::Char('a'));
        assert!(!state.needs_redraw, "a plain key release changes nothing");
    }

    #[test]
    fn focus_loss_clears_modal_repeats_and_modifiers() {
        let route_measurer = crate::draw::TextMeasurer::default();
        let route_ui_engine = crate::ui_text::UiTextEngine::default();
        let route_resources = crate::input::state::InputTextResources {
            measurer: &route_measurer,
            ui_engine: &route_ui_engine,
        };
        let mut state = make_test_input_state();
        state.toggle_command_palette();
        assert!(state.handle_command_palette_key_with_resources(route_resources, Key::Down));
        state.sync_modifiers(true, true, true, true);
        state.modifiers.tab = true;
        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_some()
        );

        state.clear_focus_owned_key_state();

        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_none()
        );
        assert!(!state.modifiers.shift);
        assert!(!state.modifiers.ctrl);
        assert!(!state.modifiers.alt);
        assert!(!state.modifiers.logo);
        assert!(!state.modifiers.tab);

        state.open_font_picker();
        assert!(state.handle_font_picker_key_with_measurer(&route_measurer, Key::Down, None));
        assert!(state.font_picker_repeat_timeout(Instant::now()).is_some());

        state.clear_focus_owned_key_state();

        assert!(state.font_picker_repeat_timeout(Instant::now()).is_none());
    }
}
