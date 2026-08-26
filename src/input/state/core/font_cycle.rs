//! Stepping the text font from the keyboard.
//!
//! `[drawing] font_cycle` is a short list of families worth reaching for while
//! presenting: prose, code, and one with serifs by default. The action walks
//! that list rather than every installed font, because a mid-demo font change is
//! a choice between two or three looks, not a font picker.
//!
//! With text selected the step restyles that text. With nothing selected it sets
//! what the next label will be written in. That is the same idiom the shape and
//! blur tools already use for their variants.

use super::InputState;
use crate::draw::{FontDescriptor, families_match};

impl InputState {
    /// Install the configured list. Blank and repeated names are the config
    /// layer's problem and have already been removed by the time this runs.
    pub fn set_font_cycle(&mut self, families: Vec<String>) {
        self.font_cycle = families;
    }

    /// The family after `current` in the list, or `None` when the list cannot
    /// offer a different one.
    ///
    /// A family that is not in the list steps to the first entry rather than
    /// nowhere: the list is where the action can go, not a claim about where the
    /// font has been.
    ///
    /// Names are matched without case, because fontconfig resolves them that
    /// way. Comparing exactly would make `sans` a family the list does not hold,
    /// and the first step would restyle nothing but the spelling.
    pub(crate) fn next_font_family(&self, current: &str) -> Option<String> {
        if self.font_cycle.is_empty() {
            return None;
        }
        let next = match self
            .font_cycle
            .iter()
            .position(|family| families_match(family, current))
        {
            Some(index) => &self.font_cycle[(index + 1) % self.font_cycle.len()],
            None => &self.font_cycle[0],
        };
        (!families_match(next, current)).then(|| next.clone())
    }

    /// Step the font and say what it landed on.
    ///
    /// The toast names the family because a font change has no visible effect
    /// until something is typed, and because a family name is the only way to
    /// tell two similar faces apart at a glance.
    pub(crate) fn cycle_font_family(&mut self) -> bool {
        if self.font_cycle.is_empty() {
            self.push_toast(
                super::ToastPriority::Info,
                FONT_CYCLE_TOAST_SOURCE,
                super::Toast::warning("No fonts configured to cycle through."),
            );
            return false;
        }

        // A selection takes the step, so the gesture edits what the user is
        // looking at rather than a setting they cannot see.
        if self.selection_has_text() {
            return self.cycle_selected_font_family();
        }

        let Some(next) = self.next_font_family(&self.font_descriptor.family) else {
            return false;
        };
        let descriptor = FontDescriptor::new(
            next.clone(),
            self.font_descriptor.weight.clone(),
            self.font_descriptor.style.clone(),
        );
        if !self.set_font_descriptor(descriptor) {
            return false;
        }
        log::info!("Text font family set to {next}");
        self.push_toast(
            super::ToastPriority::Info,
            FONT_CYCLE_TOAST_SOURCE,
            super::Toast::info(format!("Font: {next}")),
        );
        true
    }

    /// Step every selected text shape to the next family in the list.
    ///
    /// The step is decided once, from the first selected text shape, so a mixed
    /// selection converges on one family instead of fanning out further.
    fn cycle_selected_font_family(&mut self) -> bool {
        let Some(next) = self
            .first_selected_text_family()
            .and_then(|family| self.next_font_family(&family))
        else {
            return false;
        };

        let changed = self.apply_family_to_selected_text(&next);
        if changed {
            log::info!("Selected text font family set to {next}");
        }
        changed
    }
}

const FONT_CYCLE_TOAST_SOURCE: &str = "font-cycle";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Shape;
    use crate::input::state::test_support::make_test_input_state;

    fn state_with_cycle() -> InputState {
        let mut state = make_test_input_state();
        state.set_font_cycle(vec![
            "Sans".to_string(),
            "Monospace".to_string(),
            "Serif".to_string(),
        ]);
        state
    }

    #[test]
    fn the_step_walks_the_list_and_wraps_at_the_end() {
        let state = state_with_cycle();

        assert_eq!(state.next_font_family("Sans").as_deref(), Some("Monospace"));
        assert_eq!(
            state.next_font_family("Monospace").as_deref(),
            Some("Serif")
        );
        assert_eq!(state.next_font_family("Serif").as_deref(), Some("Sans"));
    }

    #[test]
    fn a_family_outside_the_list_steps_to_the_first_entry() {
        let state = state_with_cycle();

        assert_eq!(
            state.next_font_family("Comic Sans MS").as_deref(),
            Some("Sans"),
            "the list says where the action can go, not where the font has been"
        );
    }

    #[test]
    fn a_family_spelled_in_another_case_is_the_same_family() {
        let state = state_with_cycle();

        // Fontconfig resolves `sans` and `Sans` to one font. A step from the
        // first to the second would change the spelling and nothing else.
        assert_eq!(state.next_font_family("sans").as_deref(), Some("Monospace"));
        assert_eq!(state.next_font_family("SERIF").as_deref(), Some("Sans"));
    }

    #[test]
    fn a_one_entry_list_has_nowhere_to_step() {
        let mut state = make_test_input_state();
        state.set_font_cycle(vec!["Sans".to_string()]);

        assert_eq!(state.next_font_family("Sans"), None);
        assert_eq!(state.next_font_family("Serif").as_deref(), Some("Sans"));
    }

    #[test]
    fn an_empty_list_turns_the_action_off_rather_than_panicking() {
        let mut state = make_test_input_state();
        state.set_font_cycle(Vec::new());

        assert_eq!(state.next_font_family("Sans"), None);
        assert!(!state.cycle_font_family());
    }

    #[test]
    fn cycling_with_nothing_selected_sets_what_the_next_label_uses() {
        let mut state = state_with_cycle();
        let before = state.font_descriptor.family.clone();

        assert!(state.cycle_font_family());

        assert_ne!(state.font_descriptor.family, before);
        assert!(state.font_cycle.contains(&state.font_descriptor.family));
    }

    #[test]
    fn cycling_with_text_selected_restyles_that_text_and_leaves_the_tool_alone() {
        let mut state = state_with_cycle();
        let tool_font = state.font_descriptor.family.clone();
        let id = state.boards.active_frame_mut().add_shape(Shape::Text {
            x: 10,
            y: 10,
            text: "hello".to_string(),
            color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
            size: 24.0,
            font_descriptor: FontDescriptor::new(
                "Sans".to_string(),
                "normal".to_string(),
                "normal".to_string(),
            ),
            background_enabled: false,
            wrap_width: None,
        });
        state.set_selection(vec![id]);

        assert!(state.cycle_font_family());

        let frame = state.boards.active_frame();
        let Some(Shape::Text {
            font_descriptor, ..
        }) = frame.shape(id).map(|drawn| &drawn.shape)
        else {
            panic!("the text shape is still there");
        };
        assert_eq!(font_descriptor.family, "Monospace");
        assert_eq!(
            state.font_descriptor.family, tool_font,
            "restyling a selection must not also change what the next label uses"
        );
    }

    #[test]
    fn a_selection_with_no_text_in_it_falls_through_to_the_tool_font() {
        let mut state = state_with_cycle();
        let before = state.font_descriptor.family.clone();
        let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
            thick: 2.0,
        });
        state.set_selection(vec![id]);

        assert!(state.cycle_font_family());

        assert_ne!(state.font_descriptor.family, before);
    }
}
