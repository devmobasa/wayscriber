use super::color::RecolorOpacity;
use crate::draw::TextMeasurer;
use crate::draw::{Color, Shape};
use crate::input::state::core::base::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_fill(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. }
                | Shape::Ellipse { fill, .. }
                | Shape::Polygon { fill, .. } => Some(*fill),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning("No fill shapes selected."),
            );
            return false;
        };

        let result = self.apply_selection_change_with(
            measurer,
            |shape| {
                matches!(
                    shape,
                    Shape::Rect { .. } | Shape::Ellipse { .. } | Shape::Polygon { .. }
                )
            },
            |shape| match shape {
                Shape::Rect { fill, .. }
                | Shape::Ellipse { fill, .. }
                | Shape::Polygon { fill, .. }
                    if *fill != target =>
                {
                    *fill = target;
                    true
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "fill")
    }

    /// Fills every editable selected closed shape with `paint`, or turns the
    /// fill off for `None`. An opaque swatch fills at the shape's own
    /// opacity, the way the border's swatches keep it; a translucent one
    /// brings its own. Turning the fill off keeps the color, so turning it
    /// back on restores it.
    pub(crate) fn apply_selection_fill_paint_with(
        &mut self,
        measurer: &TextMeasurer,
        paint: Option<Color>,
        opacity: RecolorOpacity,
    ) -> bool {
        let result = self.apply_selection_change_with(
            measurer,
            |shape| filled(shape, paint, opacity).is_some(),
            |shape| {
                let Some(next) = filled(shape, paint, opacity) else {
                    return false;
                };
                let Some((fill, fill_color)) = fill_fields(shape) else {
                    return false;
                };
                if (*fill, *fill_color) == next {
                    return false;
                }
                (*fill, *fill_color) = next;
                true
            },
        );

        self.report_selection_apply_result(result, "fill")
    }

    /// The fill of the first editable selected closed shape, for the color
    /// picker to open on; `None` when no closed shape can be filled.
    pub(crate) fn selection_fill_paint_source(&self) -> Option<Color> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked)
            .find_map(|drawn| fill_paint(&drawn.shape))
    }

    /// Whether filling the selection with `paint` would change any shape it
    /// may edit, so picking the fill a selection already has is quiet.
    pub(crate) fn selection_fill_paint_changes(
        &self,
        paint: Option<Color>,
        opacity: RecolorOpacity,
    ) -> bool {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked)
            .filter_map(|drawn| filled(&drawn.shape, paint, opacity).zip(fill_state(&drawn.shape)))
            .any(|(next, current)| next != current)
    }
}

/// The fill a closed shape shows, or would show once filled: its own fill
/// color, or its border's.
fn fill_paint(shape: &Shape) -> Option<Color> {
    match shape {
        Shape::Rect {
            fill_color, color, ..
        }
        | Shape::Ellipse {
            fill_color, color, ..
        }
        | Shape::Polygon {
            fill_color, color, ..
        } => Some(fill_color.unwrap_or(*color)),
        _ => None,
    }
}

/// A closed shape's `(fill, fill_color)`, or `None` for a shape with no fill.
fn fill_state(shape: &Shape) -> Option<(bool, Option<Color>)> {
    match shape {
        Shape::Rect {
            fill, fill_color, ..
        }
        | Shape::Ellipse {
            fill, fill_color, ..
        }
        | Shape::Polygon {
            fill, fill_color, ..
        } => Some((*fill, *fill_color)),
        _ => None,
    }
}

/// The `(fill, fill_color)` a fill with `paint` leaves on `shape`, or `None`
/// for a shape that has no fill.
fn filled(
    shape: &Shape,
    paint: Option<Color>,
    opacity: RecolorOpacity,
) -> Option<(bool, Option<Color>)> {
    let (current_fill_color, border) = match shape {
        Shape::Rect {
            fill_color, color, ..
        }
        | Shape::Ellipse {
            fill_color, color, ..
        }
        | Shape::Polygon {
            fill_color, color, ..
        } => (*fill_color, *color),
        _ => return None,
    };
    Some(match paint {
        None => (false, current_fill_color),
        Some(swatch) => {
            let alpha = if opacity == RecolorOpacity::Swatch && swatch.a >= 1.0 {
                border.a
            } else {
                swatch.a
            };
            (true, Some(Color { a: alpha, ..swatch }))
        }
    })
}

fn fill_fields(shape: &mut Shape) -> Option<(&mut bool, &mut Option<Color>)> {
    match shape {
        Shape::Rect {
            fill, fill_color, ..
        }
        | Shape::Ellipse {
            fill, fill_color, ..
        }
        | Shape::Polygon {
            fill, fill_color, ..
        } => Some((fill, fill_color)),
        _ => None,
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

    #[test]
    fn apply_selection_fill_on_mixed_selection_turns_all_fills_on() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let rect_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            fill_color: None,
            color: state.style.current_color,
            thick: 2.0,
        });
        let ellipse_id = state.boards.active_frame_mut().add_shape(Shape::Ellipse {
            cx: 26,
            cy: 27,
            rx: 6,
            ry: 7,
            fill: true,
            fill_color: None,
            color: state.style.current_color,
            thick: 2.0,
        });
        state.set_selection(vec![rect_id, ellipse_id]);

        assert!(state.apply_selection_fill(&measurer, 0));

        match &state
            .boards
            .active_frame()
            .shape(rect_id)
            .expect("rect")
            .shape
        {
            Shape::Rect { fill, .. } => assert!(*fill),
            other => panic!("expected rect, got {other:?}"),
        }
        match &state
            .boards
            .active_frame()
            .shape(ellipse_id)
            .expect("ellipse")
            .shape
        {
            Shape::Ellipse { fill, .. } => assert!(*fill),
            other => panic!("expected ellipse, got {other:?}"),
        }
    }
}
