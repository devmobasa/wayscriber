use crate::draw::TextMeasurer;
use crate::draw::{ArrowStyle, Shape};
use crate::input::state::core::base::InputState;
use crate::input::state::core::properties::apply_selection::constants::{
    MAX_ARROW_ANGLE, MAX_ARROW_LENGTH, MIN_ARROW_ANGLE, MIN_ARROW_LENGTH,
    SELECTION_ARROW_ANGLE_STEP, SELECTION_ARROW_LENGTH_STEP,
};
use crate::input::state::{Toast, ToastPriority};
use crate::util::DEFAULT_ARROW_BEND;

/// What a restyle of the current selection should do.
///
/// "No arrows" and "arrows, but every one is locked" report differently, and
/// collapsing them into one `None` is what made a locked selection claim it
/// held no arrows.
enum ArrowStyleTarget {
    /// The selection holds no arrows at all.
    NoArrows,
    /// It holds arrows, and every one of them is locked.
    AllLocked,
    /// The style every editable arrow should end up in.
    Style(ArrowStyle),
}

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_arrow_head(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning("No arrows selected."),
            );
            return false;
        };

        let result = self.apply_selection_change_with(
            measurer,
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { head_at_end, .. } if *head_at_end != target => {
                    *head_at_end = target;
                    true
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow head")
    }

    /// Steps the style of every selected arrow.
    ///
    /// A mixed selection agrees first and steps second, matching how the
    /// boolean arrow properties resolve a mixed target: pressing once on a
    /// mixed selection is a normalization, not a jump nobody can predict.
    pub(in crate::input::state::core::properties) fn apply_selection_arrow_style(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let target = match self.selection_arrow_style_target(direction) {
            ArrowStyleTarget::NoArrows => {
                self.push_toast(
                    ToastPriority::Info,
                    "selection.apply",
                    Toast::warning("No arrows selected."),
                );
                return false;
            }
            // There is nothing to step them to, but the apply still has to run:
            // it is what counts the locked arrows, and the shared reporter needs
            // that count to say they are locked instead of claiming the
            // selection holds no arrows at all. `apply_selection_change` skips
            // locked shapes, so the target below is never written.
            ArrowStyleTarget::AllLocked => ArrowStyle::default(),
            ArrowStyleTarget::Style(style) => style,
        };

        let result = self.apply_selection_change_with(
            measurer,
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { style, bend, .. } => {
                    let mut changed = *style != target;
                    *style = target;
                    // A curved arrow drawn as something else has no arc yet, so
                    // switching to Curved would otherwise render identically to
                    // the style it replaced.
                    if target.is_curved() && *bend == 0.0 {
                        *bend = DEFAULT_ARROW_BEND;
                        changed = true;
                    }
                    changed
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow style")
    }

    /// The style a restyle should move the selection to.
    fn selection_arrow_style_target(&self, direction: i32) -> ArrowStyleTarget {
        let frame = self.boards.active_frame();
        let mut applicable = 0;
        let mut editable = Vec::new();
        for id in self.selected_shape_ids() {
            let Some(drawn) = frame.shape(*id) else {
                continue;
            };
            let Shape::Arrow { style, .. } = &drawn.shape else {
                continue;
            };
            applicable += 1;
            if !drawn.locked {
                editable.push(*style);
            }
        }

        if applicable == 0 {
            return ArrowStyleTarget::NoArrows;
        }
        let Some(first) = editable.first().copied() else {
            return ArrowStyleTarget::AllLocked;
        };
        if editable.iter().any(|style| *style != first) {
            // Mixed: land them all on one style before stepping any of them.
            return ArrowStyleTarget::Style(first);
        }
        ArrowStyleTarget::Style(if direction < 0 {
            first.previous()
        } else {
            first.next()
        })
    }

    pub(in crate::input::state::core::properties) fn apply_selection_arrow_length(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_ARROW_LENGTH_STEP * direction as f64;
        let result = self.apply_selection_change_with(
            measurer,
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_length, .. } => {
                    let next = (*arrow_length + delta).clamp(MIN_ARROW_LENGTH, MAX_ARROW_LENGTH);
                    if (next - *arrow_length).abs() > f64::EPSILON {
                        *arrow_length = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow length")
    }

    pub(in crate::input::state::core::properties) fn apply_selection_arrow_angle(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_ARROW_ANGLE_STEP * direction as f64;
        let result = self.apply_selection_change_with(
            measurer,
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_angle, .. } => {
                    let next = (*arrow_angle + delta).clamp(MIN_ARROW_ANGLE, MAX_ARROW_ANGLE);
                    if (next - *arrow_angle).abs() > f64::EPSILON {
                        *arrow_angle = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow angle")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    fn add_arrow(
        state: &mut InputState,
        head_at_end: bool,
        arrow_angle: f64,
    ) -> crate::draw::ShapeId {
        add_styled_arrow(state, head_at_end, arrow_angle, ArrowStyle::Standard, 0.0)
    }

    fn add_styled_arrow(
        state: &mut InputState,
        head_at_end: bool,
        arrow_angle: f64,
        style: ArrowStyle,
        bend: f64,
    ) -> crate::draw::ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: state.style.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle,
            head_at_end,
            style,
            bend,
            label: None,
        })
    }

    fn arrow_style(state: &InputState, id: crate::draw::ShapeId) -> ArrowStyle {
        match &state.boards.active_frame().shape(id).expect("arrow").shape {
            Shape::Arrow { style, .. } => *style,
            other => panic!("expected arrow, got {other:?}"),
        }
    }

    fn arrow_bend(state: &InputState, id: crate::draw::ShapeId) -> f64 {
        match &state.boards.active_frame().shape(id).expect("arrow").shape {
            Shape::Arrow { bend, .. } => *bend,
            other => panic!("expected arrow, got {other:?}"),
        }
    }

    #[test]
    fn restyling_several_arrows_steps_them_all_in_one_undo_entry() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let first = add_arrow(&mut state, true, 30.0);
        let second = add_arrow(&mut state, false, 30.0);
        state.set_selection(vec![first, second]);

        assert!(state.apply_selection_arrow_style(&measurer, 1));
        assert_eq!(arrow_style(&state, first), ArrowStyle::Pointy);
        assert_eq!(arrow_style(&state, second), ArrowStyle::Pointy);

        // One entry, not one per shape: a restyle is a single gesture and has
        // to come back in a single undo.
        let action = state
            .boards
            .active_frame_mut()
            .undo_last()
            .expect("restyle should be undoable");
        state.apply_action_side_effects(&action);
        assert_eq!(arrow_style(&state, first), ArrowStyle::Standard);
        assert_eq!(arrow_style(&state, second), ArrowStyle::Standard);
    }

    #[test]
    fn restyling_a_mixed_selection_makes_it_agree_before_it_steps() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let standard = add_arrow(&mut state, true, 30.0);
        let curved = add_styled_arrow(&mut state, true, 30.0, ArrowStyle::Curved, 0.3);
        state.set_selection(vec![standard, curved]);

        // First press normalizes on the first editable style rather than
        // jumping both to somewhere neither of them was.
        assert!(state.apply_selection_arrow_style(&measurer, 1));
        assert_eq!(arrow_style(&state, standard), ArrowStyle::Standard);
        assert_eq!(arrow_style(&state, curved), ArrowStyle::Standard);

        // Second press steps the now-uniform selection.
        assert!(state.apply_selection_arrow_style(&measurer, 1));
        assert_eq!(arrow_style(&state, standard), ArrowStyle::Pointy);
        assert_eq!(arrow_style(&state, curved), ArrowStyle::Pointy);
    }

    #[test]
    fn restyling_backwards_walks_the_cycle_the_other_way() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let arrow = add_arrow(&mut state, true, 30.0);
        state.set_selection(vec![arrow]);

        assert!(state.apply_selection_arrow_style(&measurer, -1));
        assert_eq!(arrow_style(&state, arrow), ArrowStyle::Double);
    }

    #[test]
    fn restyling_to_curved_gives_a_flat_arrow_an_arc_to_show() {
        let measurer = TextMeasurer::default();
        // A curved arrow at bend zero draws exactly like the style it replaced,
        // so switching to it would look like nothing happened.
        let mut state = make_state();
        let arrow = add_arrow(&mut state, true, 30.0);
        state.set_selection(vec![arrow]);

        assert!(state.apply_selection_arrow_style(&measurer, 1)); // Pointy
        assert!(state.apply_selection_arrow_style(&measurer, 1)); // Curved
        assert_eq!(arrow_style(&state, arrow), ArrowStyle::Curved);
        assert_eq!(arrow_bend(&state, arrow), DEFAULT_ARROW_BEND);
    }

    #[test]
    fn restyling_away_from_curved_and_back_keeps_the_shaped_arc() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let arrow = add_styled_arrow(&mut state, true, 30.0, ArrowStyle::Curved, 0.7);
        state.set_selection(vec![arrow]);

        // Curved -> Double -> Standard -> Pointy -> Curved.
        for _ in 0..4 {
            assert!(state.apply_selection_arrow_style(&measurer, 1));
        }
        assert_eq!(arrow_style(&state, arrow), ArrowStyle::Curved);
        assert_eq!(
            arrow_bend(&state, arrow),
            0.7,
            "the arc the user shaped was lost on the way round the cycle"
        );
    }

    #[test]
    fn restyling_reports_when_no_arrows_are_selected() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        assert!(!state.apply_selection_arrow_style(&measurer, 1));
        assert_eq!(
            state.active_toast().map(|toast| toast.message.as_str()),
            Some("No arrows selected.")
        );
    }

    #[test]
    fn cycle_arrow_style_reports_a_locked_arrow_while_its_property_control_stays_disabled() {
        // There is no style to step a locked selection to, but "no arrows"
        // is the wrong reason to give: the arrows are right there and the user
        // needs to be told to unlock them, not to select something.
        let mut state = make_state();
        let id = add_arrow(&mut state, true, 30.0);
        state
            .boards
            .active_frame_mut()
            .shape_mut(id)
            .expect("arrow")
            .locked = true;
        state.set_selection(vec![id]);

        let style_entry = state
            .build_selection_property_entries(&[id])
            .into_iter()
            .find(|entry| entry.kind == crate::input::SelectionPropertyKind::ArrowStyle)
            .expect("locked arrow style property");
        assert!(
            style_entry.disabled,
            "locked properties stay non-interactive"
        );
        assert_eq!(style_entry.value, "Locked");

        state.handle_action(crate::domain::Action::CycleArrowStyle);
        assert_eq!(
            state.active_toast().map(|toast| toast.message.as_str()),
            Some("All arrow style shapes are locked.")
        );
        assert_eq!(
            arrow_style(&state, id),
            ArrowStyle::Standard,
            "a locked arrow was restyled anyway"
        );
    }

    #[test]
    fn restyling_a_partly_locked_selection_steps_only_the_unlocked_arrows() {
        let measurer = TextMeasurer::default();
        // The locked one must not vote on the target either — it is skipped by
        // the apply, so letting it into the "are they all the same style?"
        // check would strand the selection agreeing with a shape that cannot
        // move.
        let mut state = make_state();
        let locked = add_styled_arrow(&mut state, true, 30.0, ArrowStyle::Double, 0.0);
        let editable = add_arrow(&mut state, true, 30.0);
        state
            .boards
            .active_frame_mut()
            .shape_mut(locked)
            .expect("arrow")
            .locked = true;
        state.set_selection(vec![locked, editable]);

        assert!(state.apply_selection_arrow_style(&measurer, 1));
        assert_eq!(arrow_style(&state, editable), ArrowStyle::Pointy);
        assert_eq!(
            arrow_style(&state, locked),
            ArrowStyle::Double,
            "the locked arrow was restyled"
        );
    }

    #[test]
    fn apply_selection_arrow_head_on_mixed_selection_sets_heads_to_end() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let first = add_arrow(&mut state, true, 30.0);
        let second = add_arrow(&mut state, false, 30.0);
        state.set_selection(vec![first, second]);

        assert!(state.apply_selection_arrow_head(&measurer, 0));

        for id in [first, second] {
            match &state.boards.active_frame().shape(id).expect("arrow").shape {
                Shape::Arrow { head_at_end, .. } => assert!(*head_at_end),
                other => panic!("expected arrow, got {other:?}"),
            }
        }
    }

    #[test]
    fn apply_selection_arrow_angle_clamps_to_maximum() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let arrow_id = add_arrow(&mut state, true, MAX_ARROW_ANGLE - 1.0);
        state.set_selection(vec![arrow_id]);

        assert!(state.apply_selection_arrow_angle(&measurer, 1));
        assert!(!state.apply_selection_arrow_angle(&measurer, 1));

        match &state
            .boards
            .active_frame()
            .shape(arrow_id)
            .expect("arrow")
            .shape
        {
            Shape::Arrow { arrow_angle, .. } => assert_eq!(*arrow_angle, MAX_ARROW_ANGLE),
            other => panic!("expected arrow, got {other:?}"),
        }
    }
}
