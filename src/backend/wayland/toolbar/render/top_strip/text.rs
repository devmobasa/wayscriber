use super::{ICON_TOGGLE_FONT_SIZE, TOP_LABEL_FONT_SIZE, TopStripLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::{Action, action_label, action_short_label};
use crate::input::Tool;
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, model};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::FONT_FAMILY_DEFAULT;
use super::super::widgets::*;

pub(super) fn draw_text_strip(
    layout: &mut TopStripLayout,
    mut x: f64,
    handle_w: f64,
    is_simple: bool,
    current_shape_tool: Option<Tool>,
    fill_tool_active: bool,
) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hover = layout.hover;
    let gap = layout.gap;

    let (btn_w, btn_h) = layout.spec.top_button_size();
    let y = layout.spec.top_button_y(layout.height);
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: TOP_LABEL_FONT_SIZE,
    };
    let icon_toggle_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: ICON_TOGGLE_FONT_SIZE,
    };

    for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
        let label = tool_label(tool);
        let tooltip_label = tool_tooltip_label(tool);
        let is_active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, is_active, is_hover);
        draw_label_center(ctx, label_style, x, y, btn_w, btn_h, label);
        let tooltip = layout.tool_tooltip(tool, tooltip_label);
        layout.hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::SelectTool(tool),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });
        x += btn_w + gap;
    }

    if model::top_shape_picker_visible(snapshot) && is_simple {
        let shapes_active = snapshot.shape_picker_open || current_shape_tool.is_some();
        let shapes_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, shapes_active, shapes_hover);
        draw_label_center(ctx, label_style, x, y, btn_w, btn_h, "Shapes");
        layout.hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Shapes".to_string()),
        });
        x += btn_w + gap;
    } else if model::top_shape_picker_visible(snapshot) {
        let current_polygon_tool = current_shape_tool.filter(|tool| model::is_polygon_tool(*tool));
        let polygons_active = snapshot.shape_picker_open || current_polygon_tool.is_some();
        let polygons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, polygons_active, polygons_hover);
        draw_label_center(ctx, label_style, x, y, btn_w, btn_h, "Poly");
        layout.hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Polygons".to_string()),
        });
        x += btn_w + gap;
    }

    if fill_tool_active && !snapshot.shape_picker_open && model::top_fill_visible(snapshot) {
        let fill_w = ToolbarLayoutSpec::TOP_TEXT_FILL_W;
        let fill_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, fill_w, btn_h))
            .unwrap_or(false);
        let fill_label = action_short_label(Action::ToggleFill);
        draw_checkbox(
            ctx,
            x,
            y,
            fill_w,
            btn_h,
            snapshot.fill_enabled,
            fill_hover,
            label_style,
            fill_label,
        );
        layout.hits.push(HitRegion {
            rect: (x, y, fill_w, btn_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleFill),
            )),
        });
        x += fill_w + gap;
    }

    for button in model::visible_top_utility_buttons(snapshot, is_simple, false) {
        match button {
            model::TopUtilityButton::Text => {
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                    .unwrap_or(false);
                draw_button(ctx, x, y, btn_w, btn_h, snapshot.text_active, is_hover);
                draw_label_center(
                    ctx,
                    label_style,
                    x,
                    y,
                    btn_w,
                    btn_h,
                    action_short_label(Action::EnterTextMode),
                );
                layout.hits.push(HitRegion {
                    rect: (x, y, btn_w, btn_h),
                    event: ToolbarEvent::EnterTextMode,
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        action_label(Action::EnterTextMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterTextMode),
                    )),
                });
                x += btn_w + gap;
            }
            model::TopUtilityButton::StickyNote => {
                let note_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                    .unwrap_or(false);
                draw_button(ctx, x, y, btn_w, btn_h, snapshot.note_active, note_hover);
                draw_label_center(
                    ctx,
                    label_style,
                    x,
                    y,
                    btn_w,
                    btn_h,
                    action_short_label(Action::EnterStickyNoteMode),
                );
                layout.hits.push(HitRegion {
                    rect: (x, y, btn_w, btn_h),
                    event: ToolbarEvent::EnterStickyNoteMode,
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        action_label(Action::EnterStickyNoteMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterStickyNoteMode),
                    )),
                });
                x += btn_w + gap;
            }
            model::TopUtilityButton::Screenshot => {
                let screenshot_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                    .unwrap_or(false);
                draw_button(ctx, x, y, btn_w, btn_h, false, screenshot_hover);
                draw_label_center(ctx, label_style, x, y, btn_w, btn_h, "Shot");
                layout.hits.push(HitRegion {
                    rect: (x, y, btn_w, btn_h),
                    event: ToolbarEvent::CaptureScreenshot,
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        action_label(Action::CaptureSelection),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::CaptureSelection),
                    )),
                });
                x += btn_w + gap;
            }
            model::TopUtilityButton::ClearCanvas => {
                let clear_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                    .unwrap_or(false);
                draw_button(ctx, x, y, btn_w, btn_h, false, clear_hover);
                draw_label_center(
                    ctx,
                    label_style,
                    x,
                    y,
                    btn_w,
                    btn_h,
                    action_short_label(Action::ClearCanvas),
                );
                layout.hits.push(HitRegion {
                    rect: (x, y, btn_w, btn_h),
                    event: ToolbarEvent::ClearCanvas,
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        action_label(Action::ClearCanvas),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ClearCanvas),
                    )),
                });
                x += btn_w + gap;
            }
            model::TopUtilityButton::Highlight | model::TopUtilityButton::IconMode => {}
        }
    }

    if model::top_icon_mode_toggle_visible(snapshot) {
        let icons_w = ToolbarLayoutSpec::TOP_TOGGLE_WIDTH;
        let icons_hover = hover.and_then(|(hx, hy)| {
            if point_in_rect(hx, hy, x, y, icons_w, btn_h) {
                Some(if hx < x + icons_w / 2.0 { 0 } else { 1 })
            } else {
                None
            }
        });
        let icons_active = if snapshot.use_icons { 0 } else { 1 };
        draw_segmented_control(
            ctx,
            x,
            y,
            icons_w,
            btn_h,
            ("Ico", "Txt"),
            icons_active,
            icons_hover,
            icon_toggle_style,
        );
        let half_w = icons_w / 2.0;
        layout.hits.push(HitRegion {
            rect: (x, y, half_w, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: Some("Icons mode".to_string()),
        });
        layout.hits.push(HitRegion {
            rect: (x + half_w, y, half_w, btn_h),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: Some("Text mode".to_string()),
        });
    }

    if snapshot.shape_picker_open && model::top_shape_picker_visible(snapshot) {
        let mut shape_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            draw_picker_text_row(layout, handle_w, shape_y, btn_w, btn_h, label_style, &row);
            shape_y += btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        }
    }
}

fn draw_picker_text_row(
    layout: &mut TopStripLayout,
    handle_w: f64,
    shape_y: f64,
    btn_w: f64,
    btn_h: f64,
    label_style: UiTextStyle,
    tools: &[Tool],
) {
    let ctx = layout.ctx;
    let hover = layout.hover;
    let gap = layout.gap;
    let snapshot = layout.snapshot;
    let mut shape_x = ToolbarLayoutSpec::TOP_START_X + handle_w + gap;
    for tool in tools {
        if !model::tool_visible(snapshot, *tool) {
            continue;
        }
        let label = tool_label(*tool);
        let tooltip_label = tool_tooltip_label(*tool);
        let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, shape_x, shape_y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, shape_x, shape_y, btn_w, btn_h, is_active, is_hover);
        draw_label_center(ctx, label_style, shape_x, shape_y, btn_w, btn_h, label);
        let tooltip = layout.tool_tooltip(*tool, tooltip_label);
        layout.hits.push(HitRegion {
            rect: (shape_x, shape_y, btn_w, btn_h),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });
        shape_x += btn_w + gap;
    }
}
