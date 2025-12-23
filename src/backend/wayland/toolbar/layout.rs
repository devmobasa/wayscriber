use crate::ui::toolbar::ToolbarSnapshot;

use super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::hit::HitRegion;
use crate::backend::wayland::toolbar_icons;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

#[derive(Debug, Clone, Copy)]
struct ToolbarLayoutSpec {
    use_icons: bool,
}

impl ToolbarLayoutSpec {
    const TOP_SIZE_ICONS: (u32, u32) = (735, 80);
    const TOP_SIZE_TEXT: (u32, u32) = (875, 56);
    const SIDE_WIDTH: u32 = 260;

    const TOP_GAP: f64 = 8.0;
    const TOP_START_X: f64 = 16.0;
    const TOP_ICON_BUTTON: f64 = 42.0;
    const TOP_ICON_BUTTON_Y: f64 = 6.0;
    const TOP_ICON_FILL_HEIGHT: f64 = 18.0;
    const TOP_ICON_FILL_OFFSET: f64 = 2.0;
    const TOP_TEXT_BUTTON_W: f64 = 60.0;
    const TOP_TEXT_BUTTON_H: f64 = 36.0;
    const TOP_TEXT_FILL_W: f64 = 64.0;
    const TOP_TOGGLE_WIDTH: f64 = 70.0;
    const TOP_PIN_BUTTON_SIZE: f64 = 24.0;
    const TOP_PIN_BUTTON_GAP: f64 = 6.0;
    const TOP_PIN_BUTTON_MARGIN_RIGHT: f64 = 12.0;
    const TOP_PIN_BUTTON_Y_ICON: f64 = 15.0;

    const SIDE_START_X: f64 = 16.0;
    const SIDE_START_Y: f64 = 20.0;
    const SIDE_HEADER_BUTTON_SIZE: f64 = 24.0;
    const SIDE_HEADER_BUTTON_Y: f64 = 12.0;
    const SIDE_HEADER_BUTTON_MARGIN_RIGHT: f64 = 12.0;
    const SIDE_HEADER_BUTTON_GAP: f64 = 4.0;
    const SIDE_CONTENT_PADDING_X: f64 = 32.0;
    const SIDE_COLOR_PICKER_OFFSET_Y: f64 = 28.0;
    const SIDE_COLOR_PICKER_HIT_HEIGHT: f64 = 30.0;
    const SIDE_COLOR_PICKER_EXTRA_HEIGHT: f64 = 30.0;
    const SIDE_THICKNESS_SECTION_OFFSET_Y: f64 = 120.0;
    const SIDE_SLIDER_HEIGHT: f64 = 12.0;
    const SIDE_SLIDER_STEP_Y: f64 = 24.0;
    const SIDE_NUDGE_SIZE: f64 = 24.0;
    const SIDE_TEXT_SECTION_OFFSET_Y: f64 = 40.0;
    const SIDE_ACTIONS_SECTION_OFFSET_Y: f64 = 40.0;
    const SIDE_ACTION_BUTTON_HEIGHT_ICON: f64 = 42.0;
    const SIDE_ACTION_BUTTON_HEIGHT_TEXT: f64 = 24.0;
    const SIDE_ACTION_BUTTON_GAP: f64 = 6.0;
    const SIDE_ACTION_CONTENT_GAP_TEXT: f64 = 5.0;
    const SIDE_ACTION_CONTENT_ROWS_ICON: f64 = 3.0;
    const SIDE_ACTION_CONTENT_ROWS_TEXT: f64 = 8.0;

    const SIDE_HEADER_HEIGHT: f64 = 30.0;
    const SIDE_DRAG_HANDLE_HEIGHT: f64 = 24.0;
    const SIDE_SECTION_GAP: f64 = 12.0;
    const SIDE_COLOR_SECTION_LABEL_HEIGHT: f64 = 28.0;
    const SIDE_COLOR_PICKER_INPUT_HEIGHT: f64 = 24.0;
    const SIDE_COLOR_SECTION_BOTTOM_PADDING: f64 = 8.0;
    const SIDE_COLOR_SWATCH: f64 = 24.0;
    const SIDE_COLOR_SWATCH_GAP: f64 = 6.0;
    const SIDE_SLIDER_CARD_HEIGHT: f64 = 52.0;
    const SIDE_ERASER_MODE_CARD_HEIGHT: f64 = 44.0;
    const SIDE_FONT_CARD_HEIGHT: f64 = 50.0;
    const SIDE_ACTIONS_HEADER_HEIGHT: f64 = 20.0;
    const SIDE_ACTIONS_CHECKBOX_HEIGHT: f64 = 24.0;
    const SIDE_DELAY_SECTION_HEIGHT: f64 = 55.0;
    const SIDE_TOGGLE_HEIGHT: f64 = 24.0;
    const SIDE_TOGGLE_GAP: f64 = 6.0;
    const SIDE_CUSTOM_SECTION_HEIGHT: f64 = 120.0;
    const SIDE_STEP_HEADER_HEIGHT: f64 = 20.0;
    const SIDE_FOOTER_PADDING: f64 = 20.0;

    fn new(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
        }
    }

    fn top_size(&self) -> (u32, u32) {
        if self.use_icons {
            Self::TOP_SIZE_ICONS
        } else {
            Self::TOP_SIZE_TEXT
        }
    }

    fn side_size(&self, snapshot: &ToolbarSnapshot) -> (u32, u32) {
        let base_height = Self::SIDE_HEADER_HEIGHT + Self::SIDE_DRAG_HANDLE_HEIGHT;
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

    fn top_button_size(&self) -> (f64, f64) {
        if self.use_icons {
            (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
        } else {
            (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
        }
    }

    fn top_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_ICON_BUTTON_Y
        } else {
            let (_, btn_h) = self.top_button_size();
            (height - btn_h) / 2.0
        }
    }

    fn top_pin_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_PIN_BUTTON_Y_ICON
        } else {
            (height - Self::TOP_PIN_BUTTON_SIZE) / 2.0
        }
    }

    fn top_pin_x(&self, width: f64) -> f64 {
        width
            - Self::TOP_PIN_BUTTON_SIZE * 2.0
            - Self::TOP_PIN_BUTTON_GAP
            - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }

    fn top_close_x(&self, width: f64) -> f64 {
        width - Self::TOP_PIN_BUTTON_SIZE - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }

    fn side_header_button_positions(&self, width: f64) -> (f64, f64, f64) {
        let close_x = width - Self::SIDE_HEADER_BUTTON_MARGIN_RIGHT - Self::SIDE_HEADER_BUTTON_SIZE;
        let pin_x = close_x - Self::SIDE_HEADER_BUTTON_SIZE - Self::SIDE_HEADER_BUTTON_GAP;
        (pin_x, close_x, Self::SIDE_HEADER_BUTTON_Y)
    }

    fn side_content_width(&self, width: f64) -> f64 {
        width - Self::SIDE_CONTENT_PADDING_X
    }

    fn side_color_picker_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        Self::SIDE_COLOR_PICKER_HIT_HEIGHT
            + if snapshot.show_more_colors {
                Self::SIDE_COLOR_PICKER_EXTRA_HEIGHT
            } else {
                0.0
            }
    }

    fn side_colors_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
        let rows = 1.0 + if snapshot.show_more_colors { 1.0 } else { 0.0 };
        Self::SIDE_COLOR_SECTION_LABEL_HEIGHT
            + Self::SIDE_COLOR_PICKER_INPUT_HEIGHT
            + Self::SIDE_COLOR_SECTION_BOTTOM_PADDING
            + (Self::SIDE_COLOR_SWATCH + Self::SIDE_COLOR_SWATCH_GAP) * rows
    }

    fn side_actions_content_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
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

    fn side_step_height(&self, snapshot: &ToolbarSnapshot) -> f64 {
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
    let mut y = ToolbarLayoutSpec::SIDE_START_Y;
    let x = ToolbarLayoutSpec::SIDE_START_X;
    let (pin_x, close_x, header_y) = spec.side_header_button_positions(width);
    let header_btn = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let content_width = spec.side_content_width(width);
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

    // Thickness slider
    y += ToolbarLayoutSpec::SIDE_THICKNESS_SECTION_OFFSET_Y;
    hits.push(HitRegion {
        rect: (x, y, content_width, ToolbarLayoutSpec::SIDE_SLIDER_HEIGHT),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: 0.05,
            max: 10.0,
        },
        tooltip: None,
    });
    y += ToolbarLayoutSpec::SIDE_SLIDER_STEP_Y;
    hits.push(HitRegion {
        rect: (
            x,
            y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(-0.25),
        kind: HitKind::Click,
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            x + content_width - ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(0.25),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Text size slider
    y += ToolbarLayoutSpec::SIDE_TEXT_SECTION_OFFSET_Y;
    hits.push(HitRegion {
        rect: (x, y, content_width, ToolbarLayoutSpec::SIDE_SLIDER_HEIGHT),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    // Actions section
    if snapshot.show_actions_section {
        y += ToolbarLayoutSpec::SIDE_ACTIONS_SECTION_OFFSET_Y;
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
                rect: (x, y, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: None,
            });
            y += btn_h + ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }
    }

    // Delay sliders
    if snapshot.show_delay_sliders {
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        hits.push(HitRegion {
            rect: (x, y, content_width, ToolbarLayoutSpec::SIDE_SLIDER_HEIGHT),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });
        y += ToolbarLayoutSpec::SIDE_SLIDER_STEP_Y;
        hits.push(HitRegion {
            rect: (x, y, content_width, ToolbarLayoutSpec::SIDE_SLIDER_HEIGHT),
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
        assert_eq!(picker_height, Some(30.0));

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
        assert_eq!(picker_height, Some(60.0));
    }
}
