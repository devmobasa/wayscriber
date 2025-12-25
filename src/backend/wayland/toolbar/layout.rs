use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;

use super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::hit::HitRegion;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolbarLayoutSpec {
    use_icons: bool,
    layout_mode: ToolbarLayoutMode,
    shape_picker_open: bool,
}

impl ToolbarLayoutSpec {
    pub(super) const TOP_SIZE_ICONS: (u32, u32) = (735, 80);
    pub(super) const TOP_SIZE_TEXT: (u32, u32) = (875, 56);
    pub(super) const SIDE_WIDTH: u32 = 260;

    pub(super) const TOP_GAP: f64 = 8.0;
    pub(super) const TOP_START_X: f64 = 16.0;
    pub(super) const TOP_HANDLE_SIZE: f64 = 18.0;
    pub(super) const TOP_HANDLE_Y: f64 = 10.0;
    pub(super) const TOP_ICON_BUTTON: f64 = 44.0;
    pub(super) const TOP_ICON_BUTTON_Y: f64 = 6.0;
    pub(super) const TOP_ICON_SIZE: f64 = 28.0;
    pub(super) const TOP_ICON_FILL_HEIGHT: f64 = 18.0;
    pub(super) const TOP_ICON_FILL_OFFSET: f64 = 2.0;
    pub(super) const TOP_TEXT_BUTTON_W: f64 = 60.0;
    pub(super) const TOP_TEXT_BUTTON_H: f64 = 36.0;
    pub(super) const TOP_TEXT_FILL_W: f64 = 64.0;
    pub(super) const TOP_TOGGLE_WIDTH: f64 = 70.0;
    pub(super) const TOP_PIN_BUTTON_SIZE: f64 = 24.0;
    pub(super) const TOP_PIN_BUTTON_GAP: f64 = 6.0;
    pub(super) const TOP_PIN_BUTTON_MARGIN_RIGHT: f64 = 12.0;
    pub(super) const TOP_PIN_BUTTON_Y_ICON: f64 = 15.0;
    pub(super) const TOP_SHAPE_ROW_GAP: f64 = 6.0;

    pub(super) const SIDE_START_X: f64 = 16.0;
    pub(super) const SIDE_TOP_PADDING: f64 = 12.0;
    pub(super) const SIDE_HEADER_HANDLE_SIZE: f64 = 18.0;
    pub(super) const SIDE_HEADER_HANDLE_GAP: f64 = 6.0;
    pub(super) const SIDE_HEADER_ROW_HEIGHT: f64 = 22.0;
    pub(super) const SIDE_HEADER_BOTTOM_GAP: f64 = 12.0;
    pub(super) const SIDE_HEADER_BUTTON_SIZE: f64 = 22.0;
    pub(super) const SIDE_HEADER_BUTTON_MARGIN_RIGHT: f64 = 12.0;
    pub(super) const SIDE_HEADER_BUTTON_GAP: f64 = 8.0;
    pub(super) const SIDE_HEADER_TOGGLE_WIDTH: f64 = 70.0;
    pub(super) const SIDE_HEADER_MODE_WIDTH: f64 = 78.0;
    pub(super) const SIDE_HEADER_MODE_GAP: f64 = 8.0;
    pub(super) const SIDE_CONTENT_PADDING_X: f64 = 32.0;
    pub(super) const SIDE_CARD_INSET: f64 = 6.0;
    pub(super) const SIDE_COLOR_PICKER_OFFSET_Y: f64 = 24.0;
    pub(super) const SIDE_COLOR_PICKER_EXTRA_HEIGHT: f64 = 30.0;
    pub(super) const SIDE_SLIDER_ROW_OFFSET: f64 = 26.0;
    pub(super) const SIDE_NUDGE_SIZE: f64 = 24.0;
    pub(super) const SIDE_ACTION_BUTTON_HEIGHT_ICON: f64 = 32.0;
    pub(super) const SIDE_ACTION_BUTTON_HEIGHT_TEXT: f64 = 24.0;
    pub(super) const SIDE_ACTION_BUTTON_GAP: f64 = 6.0;
    pub(super) const SIDE_ACTION_CONTENT_GAP_TEXT: f64 = 5.0;

    pub(super) const SIDE_SECTION_GAP: f64 = 12.0;
    pub(super) const SIDE_SECTION_TOGGLE_OFFSET_Y: f64 = 22.0;
    pub(super) const SIDE_COLOR_SECTION_LABEL_HEIGHT: f64 = 28.0;
    pub(super) const SIDE_COLOR_PICKER_INPUT_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_COLOR_SECTION_BOTTOM_PADDING: f64 = 8.0;
    pub(super) const SIDE_COLOR_SWATCH: f64 = 24.0;
    pub(super) const SIDE_COLOR_SWATCH_GAP: f64 = 6.0;
    pub(super) const SIDE_SECTION_LABEL_OFFSET_Y: f64 = 12.0;
    pub(super) const SIDE_SECTION_LABEL_OFFSET_TALL: f64 = 14.0;
    pub(super) const SIDE_FONT_BUTTON_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_FONT_BUTTON_GAP: f64 = 8.0;
    pub(super) const SIDE_NUDGE_ICON_SIZE: f64 = 14.0;
    pub(super) const SIDE_SLIDER_VALUE_WIDTH: f64 = 40.0;
    pub(super) const SIDE_TRACK_HEIGHT: f64 = 8.0;
    pub(super) const SIDE_TRACK_KNOB_RADIUS: f64 = 7.0;
    pub(super) const SIDE_DELAY_SLIDER_HEIGHT: f64 = 6.0;
    pub(super) const SIDE_DELAY_SLIDER_KNOB_RADIUS: f64 = 6.0;
    pub(super) const SIDE_DELAY_SLIDER_HIT_PADDING: f64 = 4.0;
    pub(super) const SIDE_DELAY_SLIDER_UNDO_OFFSET_Y: f64 = 16.0;
    pub(super) const SIDE_DELAY_SLIDER_REDO_OFFSET_Y: f64 = 32.0;
    pub(super) const SIDE_ACTION_ICON_SIZE: f64 = 18.0;
    pub(super) const SIDE_STEP_SLIDER_TOP_PADDING: f64 = 4.0;
    pub(super) const SIDE_SLIDER_CARD_HEIGHT: f64 = 52.0;
    pub(super) const SIDE_ERASER_MODE_CARD_HEIGHT: f64 = 44.0;
    pub(super) const SIDE_FONT_CARD_HEIGHT: f64 = 50.0;
    pub(super) const SIDE_DELAY_SECTION_HEIGHT: f64 = 55.0;
    pub(super) const SIDE_TOGGLE_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_TOGGLE_GAP: f64 = 6.0;
    pub(super) const SIDE_CUSTOM_SECTION_HEIGHT: f64 = 120.0;
    pub(super) const SIDE_STEP_HEADER_HEIGHT: f64 = 20.0;
    pub(super) const SIDE_PRESET_CARD_HEIGHT: f64 = 100.0;
    pub(super) const SIDE_PRESET_SLOT_SIZE: f64 = 40.0;
    pub(super) const SIDE_PRESET_SLOT_GAP: f64 = 8.0;
    pub(super) const SIDE_PRESET_ROW_OFFSET_Y: f64 = 24.0;
    pub(super) const SIDE_PRESET_ACTION_GAP: f64 = 6.0;
    pub(super) const SIDE_PRESET_ACTION_HEIGHT: f64 = 20.0;
    pub(super) const SIDE_PRESET_ACTION_BUTTON_GAP: f64 = 4.0;
    pub(super) const SIDE_FOOTER_PADDING: f64 = 10.0;
    pub(super) const SIDE_SETTINGS_BUTTON_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_SETTINGS_BUTTON_GAP: f64 = 6.0;

    pub(super) fn new(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
            layout_mode: snapshot.layout_mode,
            shape_picker_open: snapshot.shape_picker_open,
        }
    }

    pub(super) fn top_size(&self, snapshot: &ToolbarSnapshot) -> (u32, u32) {
        let base_height = if self.use_icons {
            Self::TOP_SIZE_ICONS.1
        } else {
            Self::TOP_SIZE_TEXT.1
        };
        let mut height = base_height as f64;
        if self.layout_mode == ToolbarLayoutMode::Simple && self.shape_picker_open {
            let (_, btn_h) = if self.use_icons {
                (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
            } else {
                (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
            };
            height += btn_h + Self::TOP_SHAPE_ROW_GAP;
        }

        let gap = Self::TOP_GAP;
        let btn_w = if self.use_icons {
            Self::TOP_ICON_BUTTON
        } else {
            Self::TOP_TEXT_BUTTON_W
        };
        let tool_count = if self.layout_mode == ToolbarLayoutMode::Simple {
            4
        } else {
            8
        };
        let mut x = Self::TOP_START_X + Self::TOP_HANDLE_SIZE + gap;
        x += tool_count as f64 * (btn_w + gap);
        if self.layout_mode == ToolbarLayoutMode::Simple {
            x += btn_w + gap;
        }
        let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
            || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);
        let fill_visible = !self.use_icons
            && fill_tool_active
            && !(self.layout_mode == ToolbarLayoutMode::Simple && self.shape_picker_open);
        if fill_visible {
            x += Self::TOP_TEXT_FILL_W + gap;
        }
        x += btn_w + gap; // Text button
        if self.layout_mode != ToolbarLayoutMode::Simple {
            x += btn_w + gap; // Clear
            if self.use_icons {
                x += btn_w + gap; // Highlight
            }
        }
        let left_end = x + Self::TOP_TOGGLE_WIDTH;
        let right_controls = Self::TOP_PIN_BUTTON_SIZE * 2.0
            + Self::TOP_PIN_BUTTON_GAP
            + Self::TOP_PIN_BUTTON_MARGIN_RIGHT;
        let width = left_end + gap + right_controls;

        (width.ceil() as u32, height.ceil() as u32)
    }

    pub(super) fn side_size(&self, snapshot: &ToolbarSnapshot) -> (u32, u32) {
        let base_height = self.side_content_start_y();
        let colors_h = self.side_colors_height(snapshot);
        let show_marker_opacity =
            snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
        let show_text_controls = snapshot.text_active || snapshot.show_text_controls;
        let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
        let show_presets =
            snapshot.show_presets && snapshot.preset_slot_count.min(snapshot.presets.len()) > 0;
        let show_step_section = snapshot.show_step_section;
        let show_settings_section = snapshot.show_settings_section;

        let mut height: f64 = base_height;
        let add_section = |section_height: f64, height: &mut f64| {
            if section_height > 0.0 {
                *height += section_height + Self::SIDE_SECTION_GAP;
            }
        };

        add_section(colors_h, &mut height);
        if show_presets {
            add_section(Self::SIDE_PRESET_CARD_HEIGHT, &mut height);
        }
        add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height); // Thickness
        if snapshot.thickness_targets_eraser {
            add_section(Self::SIDE_ERASER_MODE_CARD_HEIGHT, &mut height);
        }
        if show_marker_opacity {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height);
        }
        if show_text_controls {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height); // Text size
            add_section(Self::SIDE_FONT_CARD_HEIGHT, &mut height);
        }

        if show_actions {
            let actions_card_h = self.side_actions_height(snapshot);
            add_section(actions_card_h, &mut height);
        }

        if show_step_section {
            let step_h = self.side_step_height(snapshot);
            add_section(step_h, &mut height);
        }

        if show_settings_section {
            let settings_h = self.side_settings_height(snapshot);
            add_section(settings_h, &mut height);
        }

        height += Self::SIDE_FOOTER_PADDING;

        (Self::SIDE_WIDTH, height.ceil() as u32)
    }

    pub(super) fn top_button_size(&self) -> (f64, f64) {
        if self.use_icons {
            (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
        } else {
            (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
        }
    }

    pub(super) fn top_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_ICON_BUTTON_Y
        } else {
            let (_, btn_h) = self.top_button_size();
            (height - btn_h) / 2.0
        }
    }

    pub(super) fn top_pin_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_PIN_BUTTON_Y_ICON
        } else {
            (height - Self::TOP_PIN_BUTTON_SIZE) / 2.0
        }
    }

    pub(super) fn top_pin_x(&self, width: f64) -> f64 {
        width
            - Self::TOP_PIN_BUTTON_SIZE * 2.0
            - Self::TOP_PIN_BUTTON_GAP
            - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }

    pub(super) fn top_close_x(&self, width: f64) -> f64 {
        width - Self::TOP_PIN_BUTTON_SIZE - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }

    pub(super) fn side_header_button_positions(&self, width: f64) -> (f64, f64, f64) {
        let close_x = width - Self::SIDE_HEADER_BUTTON_MARGIN_RIGHT - Self::SIDE_HEADER_BUTTON_SIZE;
        let pin_x = close_x - Self::SIDE_HEADER_BUTTON_SIZE - Self::SIDE_HEADER_BUTTON_GAP;
        (pin_x, close_x, self.side_header_y())
    }

    pub(super) fn side_content_width(&self, width: f64) -> f64 {
        width - Self::SIDE_CONTENT_PADDING_X
    }

    pub(super) fn side_color_picker_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let extra = if snapshot.show_more_colors {
            Self::SIDE_COLOR_PICKER_EXTRA_HEIGHT
        } else {
            0.0
        };
        Self::SIDE_COLOR_PICKER_INPUT_HEIGHT + extra
    }

    pub(super) fn side_colors_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let rows = 1.0 + if snapshot.show_more_colors { 1.0 } else { 0.0 };
        Self::SIDE_COLOR_SECTION_LABEL_HEIGHT
            + Self::SIDE_COLOR_PICKER_INPUT_HEIGHT
            + Self::SIDE_COLOR_SECTION_BOTTOM_PADDING
            + (Self::SIDE_COLOR_SWATCH + Self::SIDE_COLOR_SWATCH_GAP) * rows
    }

    pub(super) fn side_actions_content_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let show_actions_section = snapshot.show_actions_section;
        let show_actions_advanced = snapshot.show_actions_advanced;
        if !show_actions_section && !show_actions_advanced {
            return 0.0;
        }

        let basic_count: usize = if show_actions_section { 3 } else { 0 };
        let show_delay_actions = snapshot.show_step_section && snapshot.show_delay_sliders;
        let advanced_count: usize = if show_actions_advanced {
            if show_delay_actions { 9 } else { 7 }
        } else {
            0
        };

        if self.use_icons {
            let icon_btn = Self::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = Self::SIDE_ACTION_BUTTON_GAP;
            let total_icons = basic_count + advanced_count;
            let icons_per_row = 6usize;
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            if rows > 0 {
                icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0)
            } else {
                0.0
            }
        } else {
            let action_h = Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = Self::SIDE_ACTION_CONTENT_GAP_TEXT;
            let basic_h = if basic_count > 0 {
                action_h * basic_count as f64 + action_gap * (basic_count as f64 - 1.0)
            } else {
                0.0
            };
            let advanced_rows = if advanced_count > 0 {
                advanced_count.div_ceil(2)
            } else {
                0
            };
            let advanced_h = if advanced_rows > 0 {
                action_h * advanced_rows as f64 + action_gap * (advanced_rows as f64 - 1.0)
            } else {
                0.0
            };
            let gap_between = if show_actions_section && show_actions_advanced {
                Self::SIDE_ACTION_BUTTON_GAP
            } else {
                0.0
            };
            basic_h + gap_between + advanced_h
        }
    }

    pub(super) fn side_actions_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let content = self.side_actions_content_height(snapshot);
        if content <= 0.0 {
            0.0
        } else {
            Self::SIDE_SECTION_TOGGLE_OFFSET_Y + content + Self::SIDE_ACTION_BUTTON_GAP
        }
    }

    pub(super) fn side_step_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let delay_h = if snapshot.show_delay_sliders {
            Self::SIDE_DELAY_SECTION_HEIGHT
        } else {
            0.0
        };
        let toggles_h = Self::SIDE_TOGGLE_HEIGHT * 2.0 + Self::SIDE_TOGGLE_GAP;
        Self::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + if snapshot.custom_section_enabled {
                Self::SIDE_CUSTOM_SECTION_HEIGHT
            } else {
                0.0
            }
            + delay_h
    }

    pub(super) fn side_settings_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let toggle_h = Self::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = Self::SIDE_TOGGLE_GAP;
        let mut toggle_count = 1; // Preset toasts
        if snapshot.layout_mode == ToolbarLayoutMode::Advanced {
            toggle_count += 5; // presets, actions, advanced actions, step section, text controls
        }
        let rows = (toggle_count + 1) / 2;
        let toggle_rows_h = if rows > 0 {
            toggle_h * rows as f64 + toggle_gap * (rows as f64 - 1.0)
        } else {
            0.0
        };
        let buttons_h = Self::SIDE_SETTINGS_BUTTON_HEIGHT;
        let content_h = toggle_rows_h + toggle_gap + buttons_h;
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + content_h + Self::SIDE_SETTINGS_BUTTON_GAP
    }

    pub(super) fn side_header_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING + Self::SIDE_HEADER_HANDLE_SIZE + Self::SIDE_HEADER_HANDLE_GAP
    }

    pub(super) fn side_content_start_y(&self) -> f64 {
        self.side_header_y() + Self::SIDE_HEADER_ROW_HEIGHT + Self::SIDE_HEADER_BOTTOM_GAP
    }

    pub(super) fn side_card_x(&self) -> f64 {
        Self::SIDE_START_X - Self::SIDE_CARD_INSET
    }

    pub(super) fn side_card_width(&self, width: f64) -> f64 {
        width - 2.0 * Self::SIDE_START_X + Self::SIDE_CARD_INSET * 2.0
    }
}

/// Compute the target logical size for the top toolbar given snapshot state.
pub fn top_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    ToolbarLayoutSpec::new(snapshot).top_size(snapshot)
}

/// Compute the target logical size for the side toolbar given snapshot state.
pub fn side_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    ToolbarLayoutSpec::new(snapshot).side_size(snapshot)
}

/// Populate hit regions for the top toolbar.
#[allow(dead_code)]
pub fn build_top_hits(
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let use_icons = spec.use_icons;
    let gap = ToolbarLayoutSpec::TOP_GAP;
    let mut x = ToolbarLayoutSpec::TOP_START_X;

    let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
    let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
        || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);

    if use_icons {
        let (btn_size, _) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let mut fill_anchor: Option<(f64, f64)> = None;
        let tool_buttons: &[(Tool, &str)] = if is_simple {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
            ]
        } else {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ]
        };

        let mut rect_x = None;
        let mut circle_end_x = None;
        for (tool, label) in tool_buttons {
            if *tool == Tool::Rect {
                rect_x = Some(x);
            }
            if *tool == Tool::Ellipse {
                circle_end_x = Some(x + btn_size);
            }
            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    label,
                    snapshot.binding_hints.for_tool(*tool),
                )),
            });
            x += btn_size + gap;
        }

        if is_simple {
            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
                kind: HitKind::Click,
                tooltip: Some("Shapes".to_string()),
            });
            if fill_tool_active && !snapshot.shape_picker_open {
                fill_anchor = Some((x, btn_size));
            }
            x += btn_size + gap;
        } else if let (Some(rect_x), Some(circle_end_x)) = (rect_x, circle_end_x) {
            fill_anchor = Some((rect_x, circle_end_x - rect_x));
        }

        if fill_tool_active
            && !(is_simple && snapshot.shape_picker_open)
            && let Some((fill_x, fill_w)) = fill_anchor
        {
            let fill_y = y + btn_size + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
            hits.push(HitRegion {
                rect: (
                    fill_x,
                    fill_y,
                    fill_w,
                    ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT,
                ),
                event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    "Fill",
                    snapshot.binding_hints.fill.as_deref(),
                )),
            });
        }

        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: Some(super::format_binding_label(
                "Text",
                snapshot.binding_hints.text.as_deref(),
            )),
        });
        x += btn_size + gap;

        if !is_simple {
            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::ClearCanvas,
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    "Clear",
                    snapshot.binding_hints.clear.as_deref(),
                )),
            });
            x += btn_size + gap;

            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    "Click highlight",
                    snapshot.binding_hints.toggle_highlight.as_deref(),
                )),
            });
            x += btn_size + gap;
        }

        hits.push(HitRegion {
            rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });

        if is_simple && snapshot.shape_picker_open {
            let shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
            let mut shape_x =
                ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
            let shapes: &[(Tool, &str)] = &[
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ];
            for (tool, label) in shapes {
                hits.push(HitRegion {
                    rect: (shape_x, shape_y, btn_size, btn_size),
                    event: ToolbarEvent::SelectTool(*tool),
                    kind: HitKind::Click,
                    tooltip: Some(super::format_binding_label(
                        label,
                        snapshot.binding_hints.for_tool(*tool),
                    )),
                });
                shape_x += btn_size + gap;
            }
        }
    } else {
        let (btn_w, btn_h) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let tool_buttons: &[(Tool, &str)] = if is_simple {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
            ]
        } else {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ]
        };
        for (tool, label) in tool_buttons {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    label,
                    snapshot.binding_hints.for_tool(*tool),
                )),
            });
            x += btn_w + gap;
        }

        if is_simple {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
                kind: HitKind::Click,
                tooltip: Some("Shapes".to_string()),
            });
            x += btn_w + gap;
        }

        if fill_tool_active {
            let fill_w = ToolbarLayoutSpec::TOP_TEXT_FILL_W;
            hits.push(HitRegion {
                rect: (x, y, fill_w, btn_h),
                event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    "Fill",
                    snapshot.binding_hints.fill.as_deref(),
                )),
            });
            x += fill_w + gap;
        }

        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: None,
        });
        x += btn_w + gap;

        if !is_simple {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::ClearCanvas,
                kind: HitKind::Click,
                tooltip: Some(super::format_binding_label(
                    "Clear",
                    snapshot.binding_hints.clear.as_deref(),
                )),
            });
            x += btn_w + gap;
        }

        hits.push(HitRegion {
            rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });

        if is_simple && snapshot.shape_picker_open {
            let shape_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
            let mut shape_x =
                ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
            let shapes: &[(Tool, &str)] = &[
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ];
            for (tool, label) in shapes {
                hits.push(HitRegion {
                    rect: (shape_x, shape_y, btn_w, btn_h),
                    event: ToolbarEvent::SelectTool(*tool),
                    kind: HitKind::Click,
                    tooltip: Some(super::format_binding_label(
                        label,
                        snapshot.binding_hints.for_tool(*tool),
                    )),
                });
                shape_x += btn_w + gap;
            }
        }
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = spec.top_pin_button_y(height);

    let pin_x = spec.top_pin_x(width);
    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.top_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_x = spec.top_close_x(width);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });
}

/// Populate hit regions for the side toolbar.
#[allow(dead_code)]
pub fn build_side_hits(
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let use_icons = spec.use_icons;
    let x = ToolbarLayoutSpec::SIDE_START_X;
    let (pin_x, close_x, header_y) = spec.side_header_button_positions(width);
    let header_btn = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let content_width = spec.side_content_width(width);
    let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
    let show_text_controls = snapshot.text_active || snapshot.show_text_controls;
    let icons_w = ToolbarLayoutSpec::SIDE_HEADER_TOGGLE_WIDTH;
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, header_btn),
        event: ToolbarEvent::ToggleIconMode(!snapshot.use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });
    let mode_w = ToolbarLayoutSpec::SIDE_HEADER_MODE_WIDTH;
    let mode_x = x + icons_w + ToolbarLayoutSpec::SIDE_HEADER_MODE_GAP;
    let mode_tooltip = format!(
        "Mode: S/R/A = {}/{}/{}",
        ToolbarLayoutMode::Simple.label(),
        ToolbarLayoutMode::Regular.label(),
        ToolbarLayoutMode::Advanced.label(),
    );
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, header_btn),
        event: ToolbarEvent::SetToolbarLayoutMode(snapshot.layout_mode.next()),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });
    hits.push(HitRegion {
        rect: (close_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    hits.push(HitRegion {
        rect: (pin_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let mut y = spec.side_content_start_y();

    // Color picker hit region
    let picker_y = y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_h = spec.side_color_picker_height(snapshot);
    hits.push(HitRegion {
        rect: (x, picker_y, content_width, picker_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: content_width,
            h: picker_h,
        },
        tooltip: None,
    });
    y += spec.side_colors_height(snapshot) + section_gap;

    // Preset slots
    let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if snapshot.show_presets && slot_count > 0 {
        let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
        let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
        let slot_row_y = y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
        let action_row_y = slot_row_y + slot_size + ToolbarLayoutSpec::SIDE_PRESET_ACTION_GAP;
        let action_gap = ToolbarLayoutSpec::SIDE_PRESET_ACTION_BUTTON_GAP;
        let action_w = (slot_size - action_gap) / 2.0;
        for slot_index in 0..slot_count {
            let slot = slot_index + 1;
            let slot_x = x + slot_index as f64 * (slot_size + slot_gap);
            let preset_exists = snapshot
                .presets
                .get(slot_index)
                .and_then(|preset| preset.as_ref())
                .is_some();
            if preset_exists {
                hits.push(HitRegion {
                    rect: (slot_x, slot_row_y, slot_size, slot_size),
                    event: ToolbarEvent::ApplyPreset(slot),
                    kind: HitKind::Click,
                    tooltip: Some(format!("Apply preset {}", slot)),
                });
            }
            hits.push(HitRegion {
                rect: (
                    slot_x,
                    action_row_y,
                    action_w,
                    ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
                ),
                event: ToolbarEvent::SavePreset(slot),
                kind: HitKind::Click,
                tooltip: Some(format!("Save preset {}", slot)),
            });
            if preset_exists {
                hits.push(HitRegion {
                    rect: (
                        slot_x + action_w + action_gap,
                        action_row_y,
                        action_w,
                        ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
                    ),
                    event: ToolbarEvent::ClearPreset(slot),
                    kind: HitKind::Click,
                    tooltip: Some(format!("Clear preset {}", slot)),
                });
            }
        }
        y += presets_card_h + section_gap;
    }

    // Thickness slider
    let slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let slider_hit_h = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    hits.push(HitRegion {
        rect: (x, slider_row_y, content_width, slider_hit_h),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: 1.0,
            max: 50.0,
        },
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            x,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(-1.0),
        kind: HitKind::Click,
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            x + content_width - ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(1.0),
        kind: HitKind::Click,
        tooltip: None,
    });
    y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;

    if snapshot.thickness_targets_eraser {
        y += ToolbarLayoutSpec::SIDE_ERASER_MODE_CARD_HEIGHT + section_gap;
    }

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if show_marker_opacity {
        y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;
    }

    // Text size slider
    if show_text_controls {
        let text_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
        hits.push(HitRegion {
            rect: (x, text_slider_row_y, content_width, slider_hit_h),
            event: ToolbarEvent::SetFontSize(snapshot.font_size),
            kind: HitKind::DragSetFontSize,
            tooltip: None,
        });
        y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;
        y += ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT + section_gap;
    }

    // Actions section
    let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
    if show_actions {
        let actions_card_h = spec.side_actions_height(snapshot);
        let mut action_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let basic_actions = [
            ToolbarEvent::Undo,
            ToolbarEvent::Redo,
            ToolbarEvent::ClearCanvas,
        ];
        let advanced_actions = [
            ToolbarEvent::UndoAll,
            ToolbarEvent::RedoAll,
            ToolbarEvent::UndoAllDelayed,
            ToolbarEvent::RedoAllDelayed,
            ToolbarEvent::ToggleFreeze,
            ToolbarEvent::ZoomIn,
            ToolbarEvent::ZoomOut,
            ToolbarEvent::ResetZoom,
            ToolbarEvent::ToggleZoomLock,
        ];

        if snapshot.show_actions_section {
            if use_icons {
                let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
                let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let icons_per_row = basic_actions.len();
                let total_icons_w =
                    icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
                let icons_start_x = x + (content_width - total_icons_w) / 2.0;
                for (idx, evt) in basic_actions.iter().enumerate() {
                    let bx = icons_start_x + (icon_btn + icon_gap) * idx as f64;
                    hits.push(HitRegion {
                        rect: (bx, action_y, icon_btn, icon_btn),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: None,
                    });
                }
                action_y += icon_btn;
            } else {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                for (idx, evt) in basic_actions.iter().enumerate() {
                    let by = action_y + (action_h + action_gap) * idx as f64;
                    hits.push(HitRegion {
                        rect: (x, by, content_width, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: None,
                    });
                }
                action_y += action_h * basic_actions.len() as f64
                    + action_gap * (basic_actions.len() as f64 - 1.0);
            }
        }

        if snapshot.show_actions_section && snapshot.show_actions_advanced {
            action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }

        if snapshot.show_actions_advanced {
            if use_icons {
                let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
                let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let icons_per_row = 5usize;
                let total_icons_w =
                    icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
                let icons_start_x = x + (content_width - total_icons_w) / 2.0;
                for (idx, evt) in advanced_actions.iter().enumerate() {
                    let row = idx / icons_per_row;
                    let col = idx % icons_per_row;
                    let bx = icons_start_x + (icon_btn + icon_gap) * col as f64;
                    let by = action_y + (icon_btn + icon_gap) * row as f64;
                    hits.push(HitRegion {
                        rect: (bx, by, icon_btn, icon_btn),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: None,
                    });
                }
            } else {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let action_w = (content_width - action_col_gap) / 2.0;
                for (idx, evt) in advanced_actions.iter().enumerate() {
                    let row = idx / 2;
                    let col = idx % 2;
                    let bx = x + (action_w + action_col_gap) * col as f64;
                    let by = action_y + (action_h + action_gap) * row as f64;
                    hits.push(HitRegion {
                        rect: (bx, by, action_w, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: None,
                    });
                }
            }
        }

        y += actions_card_h + section_gap;
    }

    // Delay sliders
    if snapshot.show_step_section && snapshot.show_delay_sliders {
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        let toggles_h =
            ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT * 2.0 + ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let custom_h = if snapshot.custom_section_enabled {
            ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
        } else {
            0.0
        };
        let slider_start_y = y
            + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + custom_h
            + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;
        let slider_hit_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT
            + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING * 2.0;
        let undo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
        hits.push(HitRegion {
            rect: (
                x,
                undo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
                content_width,
                slider_hit_h,
            ),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });
        let redo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
        hits.push(HitRegion {
            rect: (
                x,
                redo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
                content_width,
                slider_hit_h,
            ),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: None,
        });
    }

    if snapshot.show_step_section {
        y += spec.side_step_height(snapshot) + section_gap;
    }

    if snapshot.show_settings_section {
        let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let mut toggles: Vec<(ToolbarEvent, Option<&str>)> = vec![(
            ToolbarEvent::TogglePresetToasts(!snapshot.show_preset_toasts),
            Some("Preset toasts: apply/save/clear."),
        )];
        if snapshot.layout_mode == ToolbarLayoutMode::Advanced {
            toggles.extend_from_slice(&[
                (
                    ToolbarEvent::TogglePresets(!snapshot.show_presets),
                    Some("Presets: quick slots."),
                ),
                (
                    ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
                    Some("Actions: undo/redo/clear."),
                ),
                (
                    ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                    Some("Advanced: undo-all/delay/zoom."),
                ),
                (
                    ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                    Some("Step: step undo/redo."),
                ),
                (
                    ToolbarEvent::ToggleTextControls(!snapshot.show_text_controls),
                    Some("Text: font size/family."),
                ),
            ]);
        }

        let mut toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        for (idx, (evt, tooltip)) in toggles.iter().enumerate() {
            hits.push(HitRegion {
                rect: (x, toggle_y, content_width, toggle_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: tooltip.map(|text| text.to_string()),
            });
            if idx + 1 < toggles.len() {
                toggle_y += toggle_h + toggle_gap;
            } else {
                toggle_y += toggle_h;
            }
        }

        let buttons_y = toggle_y + toggle_gap;
        let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
        let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
        let button_w = (content_width - button_gap) / 2.0;
        hits.push(HitRegion {
            rect: (x, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigurator,
            kind: HitKind::Click,
            tooltip: Some("Config UI".to_string()),
        });
        hits.push(HitRegion {
            rect: (x + button_w + button_gap, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigFile,
            kind: HitKind::Click,
            tooltip: Some("Config file".to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardConfig, KeybindingsConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode, InputState};
    use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot};

    fn create_test_input_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings.build_action_map().unwrap();

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            3.0,
            12.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor {
                family: "Sans".to_string(),
                weight: "bold".to_string(),
                style: "normal".to_string(),
            },
            false,
            20.0,
            30.0,
            false,
            true,
            BoardConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            false,
            0,
            0,
            5,
            5,
        )
    }

    fn snapshot_from_state(state: &InputState) -> ToolbarSnapshot {
        ToolbarSnapshot::from_input_with_bindings(state, ToolbarBindingHints::default())
    }

    #[test]
    fn top_size_respects_icon_mode() {
        let mut state = create_test_input_state();
        state.toolbar_use_icons = true;
        let snapshot = snapshot_from_state(&state);
        assert_eq!(top_size(&snapshot), (758, 80));

        state.toolbar_use_icons = false;
        let snapshot = snapshot_from_state(&state);
        assert_eq!(top_size(&snapshot), (866, 56));
    }

    #[test]
    fn build_top_hits_includes_toggle_and_pin() {
        let mut state = create_test_input_state();
        state.toolbar_use_icons = true;
        let snapshot = snapshot_from_state(&state);
        let mut hits = Vec::new();
        let (w, h) = top_size(&snapshot);
        build_top_hits(w as f64, h as f64, &snapshot, &mut hits);

        assert!(
            hits.iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::ToggleIconMode(false)))
        );
        assert!(
            hits.iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::PinTopToolbar(_)))
        );
        assert!(
            hits.iter()
                .any(|hit| matches!(hit.event, ToolbarEvent::CloseTopToolbar))
        );
    }

    #[test]
    fn build_side_hits_color_picker_height_tracks_palette_mode() {
        let mut state = create_test_input_state();
        state.show_more_colors = false;
        let snapshot = snapshot_from_state(&state);
        let mut hits = Vec::new();
        build_side_hits(260.0, 400.0, &snapshot, &mut hits);
        let picker_height = hits.iter().find_map(|hit| {
            if let HitKind::PickColor { h, .. } = hit.kind {
                Some(h)
            } else {
                None
            }
        });
        assert_eq!(picker_height, Some(24.0));

        state.show_more_colors = true;
        let snapshot = snapshot_from_state(&state);
        let mut hits = Vec::new();
        build_side_hits(260.0, 400.0, &snapshot, &mut hits);
        let picker_height = hits.iter().find_map(|hit| {
            if let HitKind::PickColor { h, .. } = hit.kind {
                Some(h)
            } else {
                None
            }
        });
        assert_eq!(picker_height, Some(54.0));
    }
}
