use super::super::base::InputState;
use super::summary::{
    shape_arrow_angle, shape_arrow_head, shape_arrow_length, shape_color, shape_fill,
    shape_font_size, shape_text_background, shape_thickness, summarize_property,
};
use super::types::{SelectionPropertyEntry, SelectionPropertyKind};
use super::utils::{approx_eq, color_eq, color_label};
use crate::draw::{Shape, ShapeId};
use crate::input::state::{PressureThicknessEditMode, PressureThicknessEntryMode};

impl InputState {
    pub(super) fn build_selection_property_entries(
        &self,
        ids: &[ShapeId],
    ) -> Vec<SelectionPropertyEntry> {
        let frame = self.boards.active_frame();
        let mut entries = Vec::new();

        let color_summary = summarize_property(frame, ids, shape_color, color_eq);
        if color_summary.applicable {
            let value = if !color_summary.editable {
                "Locked".to_string()
            } else if color_summary.mixed {
                "Mixed".to_string()
            } else {
                color_summary
                    .value
                    .map(color_label)
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Color".to_string(),
                value,
                kind: SelectionPropertyKind::Color,
                disabled: !color_summary.editable,
            });
        }

        let thickness_summary = summarize_property(frame, ids, shape_thickness, approx_eq);
        if thickness_summary.applicable {
            let value = if !thickness_summary.editable {
                "Locked".to_string()
            } else if thickness_summary.mixed {
                "Mixed".to_string()
            } else {
                thickness_summary
                    .value
                    .map(|v| format!("{v:.1}px"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Thickness".to_string(),
                value,
                kind: SelectionPropertyKind::Thickness,
                disabled: !thickness_summary.editable,
            });
        } else {
            let mut any_pressure = false;
            let mut all_pressure = !ids.is_empty();
            let mut any_pressure_editable = false;
            for id in ids {
                let Some(drawn) = frame.shape(*id) else {
                    all_pressure = false;
                    continue;
                };
                if matches!(&drawn.shape, Shape::FreehandPressure { .. }) {
                    any_pressure = true;
                    if !drawn.locked {
                        any_pressure_editable = true;
                    }
                } else {
                    all_pressure = false;
                }
            }
            let show_pressure_thickness = match self.pressure_thickness_entry_mode {
                PressureThicknessEntryMode::Never => false,
                PressureThicknessEntryMode::PressureOnly => all_pressure,
                PressureThicknessEntryMode::AnyPressure => any_pressure,
            };

            if show_pressure_thickness {
                let pressure_editable = self.pressure_thickness_edit_mode
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
                    disabled: !pressure_editable,
                });
            }
        }

        let fill_summary = summarize_property(frame, ids, shape_fill, |a, b| a == b);
        if fill_summary.applicable {
            let value = if !fill_summary.editable {
                "Locked".to_string()
            } else if fill_summary.mixed {
                "Mixed".to_string()
            } else {
                fill_summary
                    .value
                    .map(|v| if v { "On" } else { "Off" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Fill".to_string(),
                value,
                kind: SelectionPropertyKind::Fill,
                disabled: !fill_summary.editable,
            });
        }

        let font_summary = summarize_property(frame, ids, shape_font_size, approx_eq);
        if font_summary.applicable {
            let value = if !font_summary.editable {
                "Locked".to_string()
            } else if font_summary.mixed {
                "Mixed".to_string()
            } else {
                font_summary
                    .value
                    .map(|v| format!("{v:.0}pt"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Font size".to_string(),
                value,
                kind: SelectionPropertyKind::FontSize,
                disabled: !font_summary.editable,
            });
        }

        let head_summary = summarize_property(frame, ids, shape_arrow_head, |a, b| a == b);
        if head_summary.applicable {
            let value = if !head_summary.editable {
                "Locked".to_string()
            } else if head_summary.mixed {
                "Mixed".to_string()
            } else {
                head_summary
                    .value
                    .map(|v| if v { "End" } else { "Start" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow head".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowHead,
                disabled: !head_summary.editable,
            });
        }

        let length_summary = summarize_property(frame, ids, shape_arrow_length, approx_eq);
        if length_summary.applicable {
            let value = if !length_summary.editable {
                "Locked".to_string()
            } else if length_summary.mixed {
                "Mixed".to_string()
            } else {
                length_summary
                    .value
                    .map(|v| format!("{v:.0}px"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow length".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowLength,
                disabled: !length_summary.editable,
            });
        }

        let angle_summary = summarize_property(frame, ids, shape_arrow_angle, approx_eq);
        if angle_summary.applicable {
            let value = if !angle_summary.editable {
                "Locked".to_string()
            } else if angle_summary.mixed {
                "Mixed".to_string()
            } else {
                angle_summary
                    .value
                    .map(|v| format!("{v:.0} deg"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow angle".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowAngle,
                disabled: !angle_summary.editable,
            });
        }

        let text_bg_summary = summarize_property(frame, ids, shape_text_background, |a, b| a == b);
        if text_bg_summary.applicable {
            let value = if !text_bg_summary.editable {
                "Locked".to_string()
            } else if text_bg_summary.mixed {
                "Mixed".to_string()
            } else {
                text_bg_summary
                    .value
                    .map(|v| if v { "On" } else { "Off" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Text background".to_string(),
                value,
                kind: SelectionPropertyKind::TextBackground,
                disabled: !text_bg_summary.editable,
            });
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            true,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    fn entry<'a>(entries: &'a [SelectionPropertyEntry], label: &str) -> &'a SelectionPropertyEntry {
        entries
            .iter()
            .find(|entry| entry.label == label)
            .expect(label)
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
            color: state.current_color,
            size: 18.0,
            font_descriptor: state.font_descriptor.clone(),
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
            color: state.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: true,
            label: None,
        });

        let entries = state.build_selection_property_entries(&[arrow_id]);

        assert_eq!(entry(&entries, "Arrow head").value, "End");
        assert_eq!(entry(&entries, "Arrow length").value, "24px");
        assert_eq!(entry(&entries, "Arrow angle").value, "35 deg");
    }

    #[test]
    fn property_entries_report_mixed_arrow_head_values() {
        let mut state = make_state();
        let first = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: state.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: true,
            label: None,
        });
        let second = state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 10,
            y1: 10,
            x2: 30,
            y2: 20,
            color: state.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle: 35.0,
            head_at_end: false,
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
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.2,
                },
                thick: 8.0,
            });

        let entries = state.build_selection_property_entries(&[marker_id]);

        assert_eq!(entry(&entries, "Color").value, "Red");
    }
}
