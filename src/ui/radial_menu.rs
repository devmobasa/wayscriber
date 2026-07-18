use std::f64::consts::PI;

use crate::input::InputState;
use crate::input::Tool;
use crate::input::state::{
    RADIAL_TOOL_LABELS, RADIAL_TOOL_SEGMENT_COUNT, RadialMenuLayout, RadialMenuState,
    RadialSegmentId, sub_ring_child_count, sub_ring_child_label,
};
use crate::ui_text::{UiTextStyle, text_layout};

use super::constants::{
    self, ACCENT_PRIMARY, BG_HOVER, BG_SELECTION, BORDER_FOCUS, DIVIDER_LIGHT, OVERLAY_DIM_LIGHT,
    PANEL_BG_CONTEXT_MENU, PROGRESS_FILL, PROGRESS_TRACK, TEXT_HINT, TEXT_SECONDARY, TEXT_WHITE,
};

// ── File-local style values without a matching token in ui/constants.rs ──

/// Rest background of tool ring wedges.
const TOOL_WEDGE_BG: (f64, f64, f64, f64) = (0.12, 0.15, 0.22, 0.88);
/// Rest background of sub-ring wedges (slightly lighter than the tool ring).
const SUB_WEDGE_BG: (f64, f64, f64, f64) = (0.15, 0.18, 0.25, 0.90);
/// Hover brighten overlay on color ring swatches.
const COLOR_HOVER_OVERLAY: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.35);
/// Border of the active color swatch.
const COLOR_ACTIVE_BORDER: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.9);
/// Border of inactive color swatches.
const COLOR_SWATCH_BORDER: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.4);

/// Separation between adjacent wedges, in pixels of arc length.
const WEDGE_GAP_PX: f64 = 2.0;
/// Tool ring wedge label font size.
const TOOL_LABEL_SIZE: f64 = 12.0;
/// Sub-ring wedge label font size.
const SUB_LABEL_SIZE: f64 = 11.0;
/// Center-well label font size.
const CENTER_LABEL_SIZE: f64 = 10.0;
/// Shortcut hint font size under tool wedge labels.
const HINT_LABEL_SIZE: f64 = 9.0;
/// Vertical lift of the wedge label when a shortcut hint is shown below it.
const HINT_LABEL_LIFT: f64 = 6.0;
/// Vertical drop of the shortcut hint below the wedge midpoint.
const HINT_LABEL_DROP: f64 = 8.0;

/// Render the radial menu overlay.
pub fn render_radial_menu(ctx: &cairo::Context, input_state: &InputState, width: u32, height: u32) {
    let (hover, expanded_sub_ring) = match &input_state.radial_menu_state {
        RadialMenuState::Open {
            hover,
            expanded_sub_ring,
            ..
        } => (*hover, *expanded_sub_ring),
        RadialMenuState::Hidden => return,
    };

    let layout = match &input_state.radial_menu_layout {
        Some(l) => *l,
        None => return,
    };

    let _ = ctx.save();

    // Semi-transparent backdrop
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_LIGHT);
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    let _ = ctx.fill();

    let cx = layout.center_x;
    let cy = layout.center_y;
    let seg_angle = 2.0 * PI / RADIAL_TOOL_SEGMENT_COUNT as f64;
    let color_count = input_state.quick_colors.radial_rendered_len();
    let color_seg_angle = if color_count > 0 {
        2.0 * PI / color_count as f64
    } else {
        0.0
    };
    // Angle offset: segment 0 starts at top, centered
    let offset = -PI / 2.0 - seg_angle / 2.0;
    let color_offset = -PI / 2.0 - color_seg_angle / 2.0;

    let active_tool = input_state.active_tool();
    let active_color = input_state.color_for_tool(active_tool);

    // ── Color ring (outermost) ──
    let color_gap = if color_count > 1 {
        gap_half_angle((layout.color_inner + layout.color_outer) / 2.0)
    } else {
        0.0
    };
    for (i, entry) in input_state
        .quick_colors
        .radial_rendered_entries()
        .iter()
        .enumerate()
    {
        let start = color_offset + i as f64 * color_seg_angle;
        let end = start + color_seg_angle;
        let is_hovered = hover == Some(RadialSegmentId::Color(i as u8));

        let c = entry.color;
        // Check if this color matches current
        let is_active = colors_match(&active_color, &c);

        draw_annular_sector(
            ctx,
            cx,
            cy,
            layout.color_inner,
            layout.color_outer,
            start + color_gap,
            end - color_gap,
        );
        ctx.set_source_rgba(c.r, c.g, c.b, c.a);
        let _ = ctx.fill_preserve();

        if is_hovered {
            constants::set_color(ctx, COLOR_HOVER_OVERLAY);
            let _ = ctx.fill_preserve();
        }

        // Border
        if is_active {
            constants::set_color(ctx, COLOR_ACTIVE_BORDER);
            ctx.set_line_width(2.5);
        } else {
            constants::set_color(ctx, COLOR_SWATCH_BORDER);
            ctx.set_line_width(1.0);
        }
        let _ = ctx.stroke();
    }

    // ── Sub-ring (if expanded) ──
    if let Some(parent_idx) = expanded_sub_ring {
        let child_count = sub_ring_child_count(parent_idx);
        if child_count > 0 {
            let parent_start = offset + parent_idx as f64 * seg_angle;
            let child_angle = seg_angle / child_count as f64;
            let sub_gap = gap_half_angle((layout.sub_inner + layout.sub_outer) / 2.0);

            for ci in 0..child_count {
                let start = parent_start + ci as f64 * child_angle;
                let end = start + child_angle;
                let is_hovered = hover == Some(RadialSegmentId::SubTool(parent_idx, ci as u8));

                draw_annular_sector(
                    ctx,
                    cx,
                    cy,
                    layout.sub_inner,
                    layout.sub_outer,
                    start + sub_gap,
                    end - sub_gap,
                );

                // Background
                if is_hovered {
                    constants::set_color(ctx, ACCENT_PRIMARY);
                } else {
                    constants::set_color(ctx, SUB_WEDGE_BG);
                }
                let _ = ctx.fill_preserve();

                // Border
                constants::set_color(ctx, DIVIDER_LIGHT);
                ctx.set_line_width(1.0);
                let _ = ctx.stroke();

                // Label
                let mid_angle = start + child_angle / 2.0;
                let mid_r = (layout.sub_inner + layout.sub_outer) / 2.0;
                let lx = cx + mid_r * mid_angle.cos();
                let ly = cy + mid_r * mid_angle.sin();
                let label = sub_ring_child_label(parent_idx, ci as u8);
                let color = if is_hovered {
                    TEXT_WHITE
                } else {
                    TEXT_SECONDARY
                };
                draw_centered_label(ctx, lx, ly, label, SUB_LABEL_SIZE, color);
            }
        }
    }

    // ── Tool ring ──
    let tool_gap = gap_half_angle((layout.tool_inner + layout.tool_outer) / 2.0);
    for (i, label) in RADIAL_TOOL_LABELS.iter().enumerate() {
        let start = offset + i as f64 * seg_angle;
        let end = start + seg_angle;
        let segment_idx = i as u8;
        let is_hovered = hover == Some(RadialSegmentId::Tool(segment_idx));
        let is_active = tool_segment_matches(segment_idx, active_tool, input_state);

        draw_annular_sector(
            ctx,
            cx,
            cy,
            layout.tool_inner,
            layout.tool_outer,
            start + tool_gap,
            end - tool_gap,
        );

        // Background
        if is_hovered {
            constants::set_color(ctx, ACCENT_PRIMARY);
        } else if is_active {
            constants::set_color(ctx, BG_SELECTION);
        } else {
            constants::set_color(ctx, TOOL_WEDGE_BG);
        }
        let _ = ctx.fill_preserve();

        // Border
        if is_active {
            constants::set_color(ctx, BORDER_FOCUS);
            ctx.set_line_width(2.0);
        } else {
            constants::set_color(ctx, DIVIDER_LIGHT);
            ctx.set_line_width(1.0);
        }
        let _ = ctx.stroke();

        // Label, with the primary bound shortcut as a dimmed hint below it
        let mid_angle = start + seg_angle / 2.0;
        let mid_r = (layout.tool_inner + layout.tool_outer) / 2.0;
        let lx = cx + mid_r * mid_angle.cos();
        let ly = cy + mid_r * mid_angle.sin();
        let label_color = if is_hovered {
            TEXT_WHITE
        } else {
            TEXT_SECONDARY
        };
        match tool_segment_hint(input_state, segment_idx) {
            Some(hint) => {
                draw_centered_label(
                    ctx,
                    lx,
                    ly - HINT_LABEL_LIFT,
                    label,
                    TOOL_LABEL_SIZE,
                    label_color,
                );
                draw_centered_label(
                    ctx,
                    lx,
                    ly + HINT_LABEL_DROP,
                    &hint,
                    HINT_LABEL_SIZE,
                    TEXT_HINT,
                );
            }
            None => draw_centered_label(ctx, lx, ly, label, TOOL_LABEL_SIZE, label_color),
        }
    }

    // ── Center circle ──
    let is_center_hovered = hover == Some(RadialSegmentId::Center);
    ctx.new_path();
    ctx.arc(cx, cy, layout.center_radius, 0.0, 2.0 * PI);
    if is_center_hovered {
        constants::set_color(ctx, BG_HOVER);
    } else {
        constants::set_color(ctx, PANEL_BG_CONTEXT_MENU);
    }
    let _ = ctx.fill_preserve();

    // Color indicator ring around center
    ctx.set_source_rgba(active_color.r, active_color.g, active_color.b, 0.9);
    ctx.set_line_width(3.0);
    let _ = ctx.stroke();

    // Current tool label in center
    let tool_label = active_tool_short_label(active_tool, input_state);
    draw_centered_label(
        ctx,
        cx,
        cy - 2.0,
        tool_label,
        CENTER_LABEL_SIZE,
        TEXT_SECONDARY,
    );

    // Thickness gauge (small arc near bottom of center)
    draw_thickness_gauge(ctx, &layout, input_state);

    let _ = ctx.restore();
}

/// Angular inset that yields half of [`WEDGE_GAP_PX`] of arc length at the
/// given radius, so adjacent wedges are separated by a full gap. Render-only:
/// hit-testing in the input layer still assigns the full segment angle.
fn gap_half_angle(radius: f64) -> f64 {
    (WEDGE_GAP_PX / 2.0) / radius.max(1.0)
}

/// Draw an annular (ring) sector path.
fn draw_annular_sector(
    ctx: &cairo::Context,
    cx: f64,
    cy: f64,
    r_inner: f64,
    r_outer: f64,
    start_angle: f64,
    end_angle: f64,
) {
    ctx.new_path();
    ctx.arc(cx, cy, r_outer, start_angle, end_angle);
    ctx.arc_negative(cx, cy, r_inner, end_angle, start_angle);
    ctx.close_path();
}

/// Draw a centered text label at the given position.
fn draw_centered_label(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    size: f64,
    color: (f64, f64, f64, f64),
) {
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    };
    let layout = text_layout(ctx, style, text, None);
    let extents = layout.ink_extents();
    let tx = x - extents.width() / 2.0 - extents.x_bearing();
    let ty = y - extents.height() / 2.0 - extents.y_bearing();
    constants::set_color(ctx, color);
    layout.show_at_baseline(ctx, tx, ty);
}

/// Primary bound shortcut for the tool a primary-ring wedge selects, mirroring
/// `dispatch_tool_segment` in the input layer. Parent wedges (Shapes, Text,
/// Actions) expand a sub-ring instead of triggering an action, so they show no
/// hint; so do wedges whose action has no binding.
fn tool_segment_hint(input_state: &InputState, idx: u8) -> Option<String> {
    let tool = match idx {
        0 => Tool::Pen,
        1 => Tool::Marker,
        2 => Tool::Line,
        3 => Tool::Arrow,
        6 => Tool::Eraser,
        7 => Tool::Select,
        _ => return None,
    };
    input_state.action_binding_primary_label(tool.action()?)
}

/// Draw a small thickness gauge arc near the center bottom.
fn draw_thickness_gauge(ctx: &cairo::Context, layout: &RadialMenuLayout, input_state: &InputState) {
    use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

    let size = input_state.size_for_active_tool();
    let frac = (size - MIN_STROKE_THICKNESS) / (MAX_STROKE_THICKNESS - MIN_STROKE_THICKNESS);
    let frac = frac.clamp(0.0, 1.0);

    let gauge_r = layout.center_radius - 6.0;
    // Arc from 0.3*PI to 0.7*PI (bottom arc)
    let arc_start = 0.3 * PI;
    let arc_end = 0.7 * PI;
    let arc_range = arc_end - arc_start;

    // Track
    ctx.new_path();
    constants::set_color(ctx, constants::with_alpha(PROGRESS_TRACK, 0.5));
    ctx.set_line_width(2.0);
    ctx.arc(
        layout.center_x,
        layout.center_y,
        gauge_r,
        arc_start,
        arc_end,
    );
    let _ = ctx.stroke();

    // Fill
    if frac > 0.01 {
        ctx.new_path();
        constants::set_color(ctx, constants::with_alpha(PROGRESS_FILL, 0.8));
        ctx.set_line_width(2.5);
        ctx.arc(
            layout.center_x,
            layout.center_y,
            gauge_r,
            arc_start,
            arc_start + arc_range * frac,
        );
        let _ = ctx.stroke();
    }
}

/// Check whether two colors are approximately equal.
fn colors_match(a: &crate::draw::Color, b: &crate::draw::Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}

/// Check whether a tool segment index matches the current active tool.
fn tool_segment_matches(idx: u8, tool: Tool, input_state: &InputState) -> bool {
    match idx {
        0 => tool == Tool::Pen,
        1 => tool == Tool::Marker,
        2 => tool == Tool::Line,
        3 => tool == Tool::Arrow,
        4 => tool == Tool::Rect || tool == Tool::Ellipse || tool == Tool::Blur,
        5 => {
            matches!(
                input_state.state,
                crate::input::DrawingState::TextInput { .. }
            ) || tool == Tool::StepMarker
        }
        6 => tool == Tool::Eraser,
        7 => tool == Tool::Select,
        _ => false,
    }
}

/// Short label for the active tool shown in the center circle.
fn active_tool_short_label(tool: Tool, input_state: &InputState) -> &'static str {
    if matches!(
        input_state.state,
        crate::input::DrawingState::TextInput { .. }
    ) {
        return "Text";
    }
    match tool {
        Tool::Pen => "Pen",
        Tool::Marker => "Marker",
        Tool::Line => "Line",
        Tool::Arrow => "Arrow",
        Tool::Rect => "Rect",
        Tool::Ellipse => "Ellipse",
        Tool::Triangle => "Triangle",
        Tool::Parallelogram => "Para",
        Tool::Rhombus => "Rhombus",
        Tool::RegularPolygon => "Polygon",
        Tool::FreeformPolygon => "Freeform",
        Tool::Blur => "Blur",
        Tool::Eraser => "Eraser",
        Tool::Select => "Select",
        Tool::Highlight => "Highlight",
        Tool::StepMarker => "Step",
    }
}
