use crate::ui::toolbar::ToolbarSnapshot;

use super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::hit::HitRegion;
use crate::backend::wayland::toolbar_icons;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolbarLayoutSpec {
    use_icons: bool,
}

impl ToolbarLayoutSpec {
    pub(super) const TOP_SIZE_ICONS: (u32, u32) = (735, 80);
    pub(super) const TOP_SIZE_TEXT: (u32, u32) = (875, 56);
    pub(super) const SIDE_WIDTH: u32 = 260;

    pub(super) const TOP_GAP: f64 = 8.0;
    pub(super) const TOP_START_X: f64 = 16.0;
    pub(super) const TOP_HANDLE_SIZE: f64 = 18.0;
    pub(super) const TOP_HANDLE_Y: f64 = 10.0;
    pub(super) const TOP_ICON_BUTTON: f64 = 42.0;
    pub(super) const TOP_ICON_BUTTON_Y: f64 = 6.0;
    pub(super) const TOP_ICON_SIZE: f64 = 26.0;
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
    pub(super) const SIDE_CONTENT_PADDING_X: f64 = 32.0;
    pub(super) const SIDE_CARD_INSET: f64 = 6.0;
    pub(super) const SIDE_COLOR_PICKER_OFFSET_Y: f64 = 24.0;
    pub(super) const SIDE_COLOR_PICKER_EXTRA_HEIGHT: f64 = 30.0;
    pub(super) const SIDE_SLIDER_ROW_OFFSET: f64 = 26.0;
    pub(super) const SIDE_NUDGE_SIZE: f64 = 24.0;
    pub(super) const SIDE_ACTION_BUTTON_HEIGHT_ICON: f64 = 42.0;
    pub(super) const SIDE_ACTION_BUTTON_HEIGHT_TEXT: f64 = 24.0;
    pub(super) const SIDE_ACTION_BUTTON_GAP: f64 = 6.0;
    pub(super) const SIDE_ACTION_CONTENT_GAP_TEXT: f64 = 5.0;
    pub(super) const SIDE_ACTION_CONTENT_ROWS_ICON: f64 = 3.0;
    pub(super) const SIDE_ACTION_CONTENT_ROWS_TEXT: f64 = 8.0;

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
    pub(super) const SIDE_ACTION_ICON_SIZE: f64 = 22.0;
    pub(super) const SIDE_STEP_SLIDER_TOP_PADDING: f64 = 4.0;
    pub(super) const SIDE_SLIDER_CARD_HEIGHT: f64 = 52.0;
    pub(super) const SIDE_ERASER_MODE_CARD_HEIGHT: f64 = 44.0;
    pub(super) const SIDE_FONT_CARD_HEIGHT: f64 = 50.0;
    pub(super) const SIDE_ACTIONS_HEADER_HEIGHT: f64 = 20.0;
    pub(super) const SIDE_ACTIONS_CHECKBOX_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_DELAY_SECTION_HEIGHT: f64 = 55.0;
    pub(super) const SIDE_TOGGLE_HEIGHT: f64 = 24.0;
    pub(super) const SIDE_TOGGLE_GAP: f64 = 6.0;
    pub(super) const SIDE_CUSTOM_SECTION_HEIGHT: f64 = 120.0;
    pub(super) const SIDE_STEP_HEADER_HEIGHT: f64 = 20.0;
    pub(super) const SIDE_FOOTER_PADDING: f64 = 20.0;

    pub(super) fn new(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
        }
    }

    pub(super) fn top_size(&self) -> (u32, u32) {
        if self.use_icons {
            Self::TOP_SIZE_ICONS
        } else {
            Self::TOP_SIZE_TEXT
        }
    }

    pub(super) fn side_size(&self, snapshot: &ToolbarSnapshot) -> (u32, u32) {
        let base_height = self.side_content_start_y();
        let colors_h = self.side_colors_height(snapshot);
        let actions_h = Self::SIDE_ACTIONS_HEADER_HEIGHT
            + Self::SIDE_ACTIONS_CHECKBOX_HEIGHT
            + self.side_actions_content_height(snapshot);
        let step_h = self.side_step_height(snapshot);

        let show_marker_opacity =
            snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;

        let mut height: f64 = base_height + colors_h + Self::SIDE_SECTION_GAP;
        height += Self::SIDE_SLIDER_CARD_HEIGHT + Self::SIDE_SECTION_GAP; // Thickness
        if snapshot.thickness_targets_eraser {
            height += Self::SIDE_ERASER_MODE_CARD_HEIGHT + Self::SIDE_SECTION_GAP; // Eraser mode
        }
        if show_marker_opacity {
            height += Self::SIDE_SLIDER_CARD_HEIGHT + Self::SIDE_SECTION_GAP; // Marker opacity
        }
        height += Self::SIDE_SLIDER_CARD_HEIGHT + Self::SIDE_SECTION_GAP; // Text size
        height += Self::SIDE_FONT_CARD_HEIGHT + Self::SIDE_SECTION_GAP;
        height += actions_h + Self::SIDE_SECTION_GAP;
        height += step_h;
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
        if !snapshot.show_actions_section {
            return 0.0;
        }
        if self.use_icons {
            (Self::SIDE_ACTION_BUTTON_HEIGHT_ICON + Self::SIDE_ACTION_BUTTON_GAP)
                * Self::SIDE_ACTION_CONTENT_ROWS_ICON
        } else {
            (Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT + Self::SIDE_ACTION_CONTENT_GAP_TEXT)
                * Self::SIDE_ACTION_CONTENT_ROWS_TEXT
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
    ToolbarLayoutSpec::new(snapshot).top_size()
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

    type IconFn = fn(&cairo::Context, f64, f64, f64);
    let buttons: &[(Tool, IconFn, &str)] = &[
        (
            Tool::Select,
            toolbar_icons::draw_icon_select as IconFn,
            "Select",
        ),
        (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn, "Pen"),
        (
            Tool::Marker,
            toolbar_icons::draw_icon_marker as IconFn,
            "Marker",
        ),
        (
            Tool::Eraser,
            toolbar_icons::draw_icon_eraser as IconFn,
            "Eraser",
        ),
        (Tool::Line, toolbar_icons::draw_icon_line as IconFn, "Line"),
        (Tool::Rect, toolbar_icons::draw_icon_rect as IconFn, "Rect"),
        (
            Tool::Ellipse,
            toolbar_icons::draw_icon_circle as IconFn,
            "Circle",
        ),
        (
            Tool::Arrow,
            toolbar_icons::draw_icon_arrow as IconFn,
            "Arrow",
        ),
    ];

    if use_icons {
        let (btn_size, _) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let _icon_size = 26.0;
        let mut rect_x = 0.0;
        let mut circle_end_x = 0.0;

        for (tool, _icon_fn, label) in buttons {
            if *tool == Tool::Rect {
                rect_x = x;
            }
            if *tool == Tool::Ellipse {
                circle_end_x = x + btn_size;
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

        let fill_y = y + btn_size + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
        let fill_w = circle_end_x - rect_x;
        hits.push(HitRegion {
            rect: (
                rect_x,
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

        let (btn_size, _) = spec.top_button_size();
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

        hits.push(HitRegion {
            rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });
    } else {
        let (btn_w, btn_h) = spec.top_button_size();
        let y = spec.top_button_y(height);

        for (tool, _icon_fn, label) in buttons {
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

        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: None,
        });
        x += btn_w + gap;

        hits.push(HitRegion {
            rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });
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
    let text_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    hits.push(HitRegion {
        rect: (x, text_slider_row_y, content_width, slider_hit_h),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });
    y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;
    y += ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT + section_gap;

    // Actions section
    let actions_card_h = ToolbarLayoutSpec::SIDE_ACTIONS_HEADER_HEIGHT
        + ToolbarLayoutSpec::SIDE_ACTIONS_CHECKBOX_HEIGHT
        + spec.side_actions_content_height(snapshot);
    if snapshot.show_actions_section {
        let mut action_y = y
            + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y
            + ToolbarLayoutSpec::SIDE_ACTIONS_CHECKBOX_HEIGHT
            + ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let actions: &[(ToolbarEvent, bool)] = &[
            (ToolbarEvent::Undo, true),
            (ToolbarEvent::Redo, true),
            (ToolbarEvent::UndoAll, true),
            (ToolbarEvent::RedoAll, true),
            (ToolbarEvent::ClearCanvas, true),
            (ToolbarEvent::ToggleFreeze, false),
        ];
        let btn_h = if use_icons {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        let btn_w = content_width;
        for (evt, _) in actions {
            hits.push(HitRegion {
                rect: (x, action_y, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: None,
            });
            action_y += btn_h + ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }
    }
    y += actions_card_h + section_gap;

    // Delay sliders
    if snapshot.show_delay_sliders {
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
        assert_eq!(top_size(&snapshot), (735, 80));

        state.toolbar_use_icons = false;
        let snapshot = snapshot_from_state(&state);
        assert_eq!(top_size(&snapshot), (875, 56));
    }

    #[test]
    fn build_top_hits_includes_toggle_and_pin() {
        let mut state = create_test_input_state();
        state.toolbar_use_icons = true;
        let snapshot = snapshot_from_state(&state);
        let mut hits = Vec::new();
        build_top_hits(735.0, 80.0, &snapshot, &mut hits);

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
