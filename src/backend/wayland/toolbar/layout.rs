use crate::ui::toolbar::ToolbarSnapshot;

use super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::hit::HitRegion;
use crate::backend::wayland::toolbar_icons;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

/// Compute the target logical size for the top toolbar given snapshot state.
pub fn top_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    if snapshot.use_icons {
        (735, 80)
    } else {
        (875, 56)
    }
}

/// Compute the target logical size for the side toolbar given snapshot state.
pub fn side_size(snapshot: &ToolbarSnapshot) -> (u32, u32) {
    let base_height = 30.0 + 24.0; // Header + drag handle
    let section_gap = 12.0;

    let picker_h = 24.0;
    let swatch = 24.0;
    let swatch_gap = 6.0;
    let basic_rows = 1.0;
    let extended_rows = if snapshot.show_more_colors { 1.0 } else { 0.0 };
    let colors_h = 28.0 + picker_h + 8.0 + (swatch + swatch_gap) * (basic_rows + extended_rows);

    let slider_card_h = 52.0;
    let font_card_h = 50.0;

    let actions_checkbox_h = 24.0;
    let actions_content_h = if snapshot.show_actions_section {
        if snapshot.use_icons {
            let icon_btn_size = 42.0;
            let icon_gap = 6.0;
            let icon_rows = 3.0;
            (icon_btn_size + icon_gap) * icon_rows
        } else {
            let action_h = 24.0;
            let action_gap = 5.0;
            let action_rows = 8.0;
            (action_h + action_gap) * action_rows
        }
    } else {
        0.0
    };
    let actions_h = 20.0 + actions_checkbox_h + actions_content_h;

    let delay_h = if snapshot.show_delay_sliders {
        55.0
    } else {
        0.0
    };
    let toggle_h = 24.0;
    let toggle_gap = 6.0;
    let toggles_h = toggle_h * 2.0 + toggle_gap;
    let step_h = 20.0
        + toggles_h
        + if snapshot.custom_section_enabled {
            120.0
        } else {
            0.0
        }
        + delay_h;

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;

    let mut height: f64 = base_height + colors_h + section_gap;
    height += slider_card_h + section_gap; // Thickness
    if show_marker_opacity {
        height += slider_card_h + section_gap; // Marker opacity
    }
    height += slider_card_h + section_gap; // Text size
    height += font_card_h + section_gap;
    height += actions_h + section_gap;
    height += step_h;
    height += 20.0;

    (260, height.ceil() as u32)
}

/// Populate hit regions for the top toolbar.
#[allow(dead_code)]
pub fn build_top_hits(
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let use_icons = snapshot.use_icons;
    let gap = 8.0;
    let mut x = 16.0;

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
        let btn_size = 42.0;
        let y = 6.0;
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

        let fill_y = y + btn_size + 2.0;
        let fill_w = circle_end_x - rect_x;
        hits.push(HitRegion {
            rect: (rect_x, fill_y, fill_w, 18.0),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(super::format_binding_label(
                "Fill",
                snapshot.binding_hints.fill.as_deref(),
            )),
        });

        let btn_size = 42.0;
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
            rect: (x, y, 70.0, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });
    } else {
        let btn_w = 60.0;
        let btn_h = 36.0;
        let y = (height - btn_h) / 2.0;

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

        let fill_w = 64.0;
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
            rect: (x, y, 70.0, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });
    }

    let btn_size = 24.0;
    let btn_gap = 6.0;
    let btn_y = if use_icons {
        15.0
    } else {
        (height - btn_size) / 2.0
    };

    let pin_x = width - btn_size * 2.0 - btn_gap - 12.0;
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

    let close_x = width - btn_size - 12.0;
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
    let use_icons = snapshot.use_icons;
    let mut y = 20.0;
    let x = 16.0;
    hits.push(HitRegion {
        rect: (width - 36.0, 12.0, 24.0, 24.0),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    hits.push(HitRegion {
        rect: (width - 64.0, 12.0, 24.0, 24.0),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    // Color picker hit region
    hits.push(HitRegion {
        rect: (
            x,
            y + 28.0,
            width - 32.0,
            30.0 + if snapshot.show_more_colors { 30.0 } else { 0.0 },
        ),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: y + 28.0,
            w: width - 32.0,
            h: 30.0 + if snapshot.show_more_colors { 30.0 } else { 0.0 },
        },
        tooltip: None,
    });

    // Thickness slider
    y += 120.0;
    hits.push(HitRegion {
        rect: (x, y, width - 32.0, 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: 0.05,
            max: 10.0,
        },
        tooltip: None,
    });
    y += 24.0;
    hits.push(HitRegion {
        rect: (x, y, 24.0, 24.0),
        event: ToolbarEvent::NudgeThickness(-0.25),
        kind: HitKind::Click,
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (x + (width - 32.0) - 24.0, y, 24.0, 24.0),
        event: ToolbarEvent::NudgeThickness(0.25),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Text size slider
    y += 40.0;
    hits.push(HitRegion {
        rect: (x, y, width - 32.0, 12.0),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    // Actions section
    if snapshot.show_actions_section {
        y += 40.0;
        let actions: &[(ToolbarEvent, bool)] = &[
            (ToolbarEvent::Undo, true),
            (ToolbarEvent::Redo, true),
            (ToolbarEvent::UndoAll, true),
            (ToolbarEvent::RedoAll, true),
            (ToolbarEvent::ClearCanvas, true),
            (ToolbarEvent::ToggleFreeze, false),
        ];
        let btn_h = if use_icons { 42.0 } else { 24.0 };
        let btn_w = width - 32.0;
        for (evt, _) in actions {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: None,
            });
            y += btn_h + 6.0;
        }
    }

    // Delay sliders
    if snapshot.show_delay_sliders {
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        hits.push(HitRegion {
            rect: (x, y, width - 32.0, 12.0),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });
        y += 24.0;
        hits.push(HitRegion {
            rect: (x, y, width - 32.0, 12.0),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: None,
        });
    }
}
