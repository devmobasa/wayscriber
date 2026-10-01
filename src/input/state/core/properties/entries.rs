use super::super::base::InputState;
use super::summary::{
    PropertySummary, resolve_selected_shapes, shape_arrow_angle, shape_arrow_head,
    shape_arrow_length, shape_arrow_style, shape_color, shape_fill_paint, shape_font_size,
    shape_opacity, shape_spotlight_magnification, shape_text_background, shape_thickness,
    summarize_property,
};
use super::types::{SelectionPropertyEntry, SelectionPropertyKind, SelectionPropertyValue};
use super::utils::{approx_eq, color_label, color_rgba_eq};
use crate::draw::{Shape, ShapeId};
use crate::input::state::{PressureThicknessEditMode, PressureThicknessEntryMode};

/// Renders one summary the way every popup row shows it: a locked row reads
/// "Locked", a mixed row "Mixed", and anything else formats its single value.
///
/// Every property repeats this shape, so it lives here once — a new property
/// that formats its value differently still cannot get the locked/mixed
/// wording wrong.
fn summary_value<T: Clone>(
    summary: &PropertySummary<T>,
    format: impl FnOnce(T) -> String,
) -> String {
    if !summary.editable {
        return "Locked".to_string();
    }
    if summary.mixed {
        return "Mixed".to_string();
    }
    summary
        .value
        .clone()
        .map(format)
        .unwrap_or_else(|| "Mixed".to_string())
}

/// The summary's single editable value, or `None` when it is mixed or locked:
/// the typed twin of [`summary_value`].
fn summary_single<T: Clone>(summary: &PropertySummary<T>) -> Option<T> {
    if !summary.editable || summary.mixed {
        return None;
    }
    summary.value.clone()
}

fn entry<T: Clone>(
    label: &str,
    kind: SelectionPropertyKind,
    summary: &PropertySummary<T>,
    format: impl FnOnce(T) -> String,
    state: impl FnOnce(Option<T>) -> SelectionPropertyValue,
) -> SelectionPropertyEntry {
    SelectionPropertyEntry {
        label: label.to_string(),
        value: summary_value(summary, format),
        kind,
        state: state(summary_single(summary)),
        disabled: !summary.editable,
    }
}

impl InputState {
    /// Whether some selected shape has a color that is not locked.
    pub(crate) fn selection_has_editable_color(&self) -> bool {
        let frame = self.boards.active_frame();
        summarize_property(
            &resolve_selected_shapes(frame, self.selected_shape_ids()),
            shape_color,
            color_rgba_eq,
        )
        .editable
    }

    pub(super) fn build_selection_property_entries(
        &self,
        ids: &[ShapeId],
    ) -> Vec<SelectionPropertyEntry> {
        let frame = self.boards.active_frame();
        let selected = resolve_selected_shapes(frame, ids);
        let palette = self.style.quick_colors.rendered_entries();
        let mut entries = Vec::new();

        // Opacity counts: two reds at different opacities are a mixed color.
        let color_summary = summarize_property(&selected, shape_color, color_rgba_eq);
        if color_summary.applicable {
            entries.push(entry(
                "Color",
                SelectionPropertyKind::Color,
                &color_summary,
                |color| color_label(palette, color),
                SelectionPropertyValue::Color,
            ));
        }

        let thickness_summary = summarize_property(&selected, shape_thickness, approx_eq);
        if thickness_summary.applicable {
            entries.push(entry(
                "Thickness",
                SelectionPropertyKind::Thickness,
                &thickness_summary,
                |v| format!("{v:.1}px"),
                SelectionPropertyValue::Level,
            ));
        } else {
            let mut any_pressure = false;
            let mut all_pressure = !ids.is_empty() && selected.len() == ids.len();
            let mut any_pressure_editable = false;
            for drawn in &selected {
                if matches!(&drawn.shape, Shape::FreehandPressure { .. }) {
                    any_pressure = true;
                    if !drawn.locked {
                        any_pressure_editable = true;
                    }
                } else {
                    all_pressure = false;
                }
            }
            let show_pressure_thickness = match self.style.pressure_thickness_entry_mode {
                PressureThicknessEntryMode::Never => false,
                PressureThicknessEntryMode::PressureOnly => all_pressure,
                PressureThicknessEntryMode::AnyPressure => any_pressure,
            };

            if show_pressure_thickness {
                let pressure_editable = self.style.pressure_thickness_edit_mode
                    != PressureThicknessEditMode::Disabled
                    && any_pressure_editable;
                entries.push(SelectionPropertyEntry {
                    label: "Thickness".to_string(),
                    value: if any_pressure_editable {
                        "Varies (pressure)".to_string()
                    } else {
                        "Locked".to_string()
                    },
                    kind: SelectionPropertyKind::Thickness,
                    state: SelectionPropertyValue::PressureVaries,
                    disabled: !pressure_editable,
                });
            }
        }

        let opacity_summary = summarize_property(&selected, shape_opacity, approx_eq);
        if opacity_summary.applicable {
            entries.push(entry(
                "Opacity",
                SelectionPropertyKind::Opacity,
                &opacity_summary,
                |v| format!("{:.0}%", v * 100.0),
                SelectionPropertyValue::Level,
            ));
        }

        let fill_summary = summarize_property(&selected, shape_fill_paint, |a, b| match (a, b) {
            (Some(a), Some(b)) => color_rgba_eq(a, b),
            (a, b) => a.is_none() && b.is_none(),
        });
        if fill_summary.applicable {
            entries.push(entry(
                "Fill",
                SelectionPropertyKind::Fill,
                &fill_summary,
                |paint| {
                    paint.map_or_else(|| "None".to_string(), |color| color_label(palette, color))
                },
                SelectionPropertyValue::Fill,
            ));
        }

        let font_summary = summarize_property(&selected, shape_font_size, approx_eq);
        if font_summary.applicable {
            entries.push(entry(
                "Font size",
                SelectionPropertyKind::FontSize,
                &font_summary,
                |v| format!("{v:.0}pt"),
                SelectionPropertyValue::Number,
            ));
        }

        let head_summary = summarize_property(&selected, shape_arrow_head, |a, b| a == b);
        if head_summary.applicable {
            entries.push(entry(
                "Arrow head",
                SelectionPropertyKind::ArrowHead,
                &head_summary,
                |v| if v { "End" } else { "Start" }.to_string(),
                SelectionPropertyValue::ArrowHead,
            ));
        }

        let style_summary = summarize_property(&selected, shape_arrow_style, |a, b| a == b);
        if style_summary.applicable {
            entries.push(entry(
                "Arrow style",
                SelectionPropertyKind::ArrowStyle,
                &style_summary,
                |v| v.label().to_string(),
                SelectionPropertyValue::ArrowStyle,
            ));
        }

        let length_summary = summarize_property(&selected, shape_arrow_length, approx_eq);
        if length_summary.applicable {
            entries.push(entry(
                "Arrow length",
                SelectionPropertyKind::ArrowLength,
                &length_summary,
                |v| format!("{v:.0}px"),
                SelectionPropertyValue::Number,
            ));
        }

        let angle_summary = summarize_property(&selected, shape_arrow_angle, approx_eq);
        if angle_summary.applicable {
            entries.push(entry(
                "Arrow angle",
                SelectionPropertyKind::ArrowAngle,
                &angle_summary,
                |v| format!("{v:.0}°"),
                SelectionPropertyValue::Number,
            ));
        }

        let text_bg_summary = summarize_property(&selected, shape_text_background, |a, b| a == b);
        if text_bg_summary.applicable {
            entries.push(entry(
                "Text background",
                SelectionPropertyKind::TextBackground,
                &text_bg_summary,
                |v| if v { "On" } else { "Off" }.to_string(),
                SelectionPropertyValue::Toggle,
            ));
        }

        let spotlight_summary =
            summarize_property(&selected, shape_spotlight_magnification, approx_eq);
        if spotlight_summary.applicable {
            entries.push(entry(
                "Magnification",
                SelectionPropertyKind::SpotlightMagnification,
                &spotlight_summary,
                crate::draw::format_spotlight_magnification,
                SelectionPropertyValue::Number,
            ));
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::color::{PALETTE_GREEN, PALETTE_RED};
    use crate::draw::{ArrowStyle, Color};

    fn make_state() -> InputState {
        crate::input::state::test_support::TestInputStateBuilder::default()
            .text_background_enabled(true)
            .build()
    }

    fn entry<'a>(entries: &'a [SelectionPropertyEntry], label: &str) -> &'a SelectionPropertyEntry {
        entries
            .iter()
            .find(|entry| entry.label == label)
            .expect(label)
    }

    #[test]
    fn selection_pill_resolves_sparse_and_all_selected_frames_without_id_rescans() {
        let mut state = make_state();
        let ids: Vec<_> = (0..2_048)
            .map(|index| {
                state.boards.active_frame_mut().add_shape(Shape::Rect {
                    x: index * 10,
                    y: 0,
                    w: 10,
                    h: 10,
                    fill: false,
                    fill_color: None,
                    color: PALETTE_RED,
                    thick: 3.0,
                })
            })
            .collect();

        for selected in [vec![ids[2], ids[1_000], ids[2_047]], ids] {
            state.set_selection(selected);
            crate::draw::Frame::reset_linear_id_lookup_count();
            let entries = state.selection_pill_entries();

            assert_eq!(entry(&entries, "Thickness").value, "3.0px");
            assert_eq!(entry(&entries, "Opacity").value, "100%");
            assert!(!entry(&entries, "Color").disabled);
            assert_eq!(crate::draw::Frame::linear_id_lookup_count(), 0);
        }
    }

    #[test]
    fn property_entries_report_mixed_color_for_different_rectangles() {
        let mut state = make_state();
        let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            fill_color: None,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });
        let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 20,
            y: 20,
            w: 10,
            h: 10,
            fill: false,
            fill_color: None,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 2.0,
        });

        let entries = state.build_selection_property_entries(&[first, second]);
        let color = entry(&entries, "Color");

        assert_eq!(color.value, "Mixed");
        assert!(!color.disabled);
    }

    #[test]
    fn property_entries_mark_locked_text_properties_as_locked() {
        let mut state = make_state();
        let text_id = state.boards.active_frame_mut().add_shape(Shape::Text {
            x: 40,
            y: 60,
            text: "Locked".to_string(),
            color: state.style.current_color,
            size: 18.0,
            font_descriptor: state.style.font_descriptor.clone(),
            background_enabled: true,
            wrap_width: None,
        });
        let index = state
            .boards
            .active_frame()
            .find_index(text_id)
            .expect("text index");
        state.boards.active_frame_mut().shapes[index].locked = true;

        let entries = state.build_selection_property_entries(&[text_id]);

        assert_eq!(entry(&entries, "Color").value, "Locked");
        assert!(entry(&entries, "Color").disabled);
        assert_eq!(entry(&entries, "Font size").value, "Locked");
        assert!(entry(&entries, "Font size").disabled);
        assert_eq!(entry(&entries, "Text background").value, "Locked");
        assert!(entry(&entries, "Text background").disabled);
    }

    #[test]
    fn property_entries_format_arrow_values_for_single_arrow() {
        let mut state = make_state();
        let arrow_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: state.style.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: true,
            style: ArrowStyle::Standard,
            bend: 0.0,
            label: None,
        });

        let entries = state.build_selection_property_entries(&[arrow_id]);

        assert_eq!(entry(&entries, "Arrow head").value, "End");
        assert_eq!(entry(&entries, "Arrow length").value, "24px");
        assert_eq!(entry(&entries, "Arrow angle").value, "35°");
    }

    #[test]
    fn property_entries_report_mixed_arrow_head_values() {
        let mut state = make_state();
        let first = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: state.style.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: true,
            style: ArrowStyle::Standard,
            bend: 0.0,
            label: None,
        });
        let second = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 10,
            y1: 10,
            x2: 30,
            y2: 20,
            color: state.style.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: false,
            style: ArrowStyle::Standard,
            bend: 0.0,
            label: None,
        });

        let entries = state.build_selection_property_entries(&[first, second]);

        assert_eq!(entry(&entries, "Arrow head").value, "Mixed");
    }

    #[test]
    fn property_entries_treat_marker_alpha_as_opaque_for_palette_labels() {
        let mut state = make_state();
        let marker_id = state
            .boards
            .active_frame_mut()
            .add_shape(Shape::MarkerStroke {
                points: vec![(0, 0), (10, 10)],
                color: Color {
                    a: 0.2,
                    ..PALETTE_RED
                },
                thick: 8.0,
            });

        let entries = state.build_selection_property_entries(&[marker_id]);

        assert_eq!(entry(&entries, "Color").value, "Red");
    }

    #[test]
    fn property_entries_name_colors_from_the_quick_color_palette() {
        let mut state = make_state();
        let palette_green = add_line(&mut state, PALETTE_GREEN);
        let pure_green = add_line(&mut state, crate::draw::GREEN);

        let entries = state.build_selection_property_entries(&[palette_green]);
        assert_eq!(entry(&entries, "Color").value, "Green");
        assert_eq!(
            entry(&entries, "Color").state,
            SelectionPropertyValue::Color(Some(PALETTE_GREEN))
        );

        let entries = state.build_selection_property_entries(&[pure_green]);
        assert_eq!(
            entry(&entries, "Color").value,
            "Custom",
            "a color the palette does not hold has no palette name"
        );
    }

    #[test]
    fn property_entries_carry_typed_values_for_controls() {
        let mut state = make_state();
        let arrow_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: PALETTE_RED,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: false,
            style: ArrowStyle::Pointy,
            bend: 0.0,
            label: None,
        });

        let entries = state.build_selection_property_entries(&[arrow_id]);

        assert_eq!(
            entry(&entries, "Thickness").state,
            SelectionPropertyValue::Level(Some(3.0))
        );
        assert_eq!(
            entry(&entries, "Arrow head").state,
            SelectionPropertyValue::ArrowHead(Some(false))
        );
        assert_eq!(
            entry(&entries, "Arrow style").state,
            SelectionPropertyValue::ArrowStyle(Some(ArrowStyle::Pointy))
        );
        assert_eq!(
            entry(&entries, "Arrow length").state,
            SelectionPropertyValue::Number(Some(24.0))
        );
    }

    #[test]
    fn mixed_and_locked_properties_carry_no_typed_value() {
        let mut state = make_state();
        let first = add_line(&mut state, PALETTE_RED);
        let second = add_line(&mut state, PALETTE_GREEN);

        let entries = state.build_selection_property_entries(&[first, second]);
        assert_eq!(
            entry(&entries, "Color").state,
            SelectionPropertyValue::Color(None)
        );

        for id in [first, second] {
            let index = state
                .boards
                .active_frame()
                .find_index(id)
                .expect("line index");
            state.boards.active_frame_mut().shapes[index].locked = true;
        }
        let entries = state.build_selection_property_entries(&[first, second]);
        assert_eq!(
            entry(&entries, "Thickness").state,
            SelectionPropertyValue::Level(None)
        );
        assert!(entry(&entries, "Thickness").disabled);
    }

    fn add_line(state: &mut InputState, color: Color) -> ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 40,
            y2: 10,
            color,
            thick: 3.0,
        })
    }
}
