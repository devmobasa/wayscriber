use super::super::base::InputState;
use super::summary::{
    shape_arrow_angle, shape_arrow_head, shape_arrow_length, shape_color, shape_fill,
    shape_font_size, shape_text_background, shape_thickness, summarize_property,
};
use super::types::{SelectionPropertyEntry, SelectionPropertyKind};
use super::utils::{approx_eq, color_eq, color_label};
use crate::draw::ShapeId;

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
