use std::f64::consts::PI;

use crate::input::InputState;
use crate::input::Tool;
use crate::input::state::{
    RADIAL_COLOR_SEGMENT_COUNT, RADIAL_TOOL_LABELS, RADIAL_TOOL_SEGMENT_COUNT, RadialMenuLayout,
    RadialMenuState, RadialSegmentId, radial_color_for_index, sub_ring_child_count,
    sub_ring_child_label,
};

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
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    let _ = ctx.fill();

    let cx = layout.center_x;
    let cy = layout.center_y;
    let seg_angle = 2.0 * PI / RADIAL_TOOL_SEGMENT_COUNT as f64;
    let color_seg_angle = 2.0 * PI / RADIAL_COLOR_SEGMENT_COUNT as f64;
    // Angle offset: segment 0 starts at top, centered
    let offset = -PI / 2.0 - seg_angle / 2.0;
    let color_offset = -PI / 2.0 - color_seg_angle / 2.0;

    let active_tool = input_state.active_tool();

    // ── Color ring (outermost) ──
    for i in 0..RADIAL_COLOR_SEGMENT_COUNT {
        let start = color_offset + i as f64 * color_seg_angle;
        let end = start + color_seg_angle;
        let is_hovered = hover == Some(RadialSegmentId::Color(i as u8));

        let c = radial_color_for_index(i as u8);
        // Check if this color matches current
        let is_active = colors_match(&input_state.current_color, &c);

        draw_annular_sector(
            ctx,
            cx,
            cy,
            layout.color_inner,
            layout.color_outer,
            start,
            end,
        );
        ctx.set_source_rgba(c.r, c.g, c.b, c.a);
        let _ = ctx.fill_preserve();

        if is_hovered {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.35);
            let _ = ctx.fill_preserve();
        }

        // Border
        if is_active {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            ctx.set_line_width(2.5);
        } else {
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4);
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

            for ci in 0..child_count {
                let start = parent_start + ci as f64 * child_angle;
                let end = start + child_angle;
                let is_hovered = hover == Some(RadialSegmentId::SubTool(parent_idx, ci as u8));

                draw_annular_sector(ctx, cx, cy, layout.sub_inner, layout.sub_outer, start, end);

                // Background
                if is_hovered {
                    ctx.set_source_rgba(0.30, 0.50, 0.80, 0.85);
                } else {
                    ctx.set_source_rgba(0.15, 0.18, 0.25, 0.90);
                }
                let _ = ctx.fill_preserve();

                // Border
                ctx.set_source_rgba(0.35, 0.40, 0.50, 0.7);
                ctx.set_line_width(1.0);
                let _ = ctx.stroke();

                // Label
                let mid_angle = start + child_angle / 2.0;
                let mid_r = (layout.sub_inner + layout.sub_outer) / 2.0;
                let lx = cx + mid_r * mid_angle.cos();
                let ly = cy + mid_r * mid_angle.sin();
                let label = sub_ring_child_label(parent_idx, ci as u8);
                draw_centered_label(ctx, lx, ly, label, 10.0, is_hovered);
            }
        }
    }

    // ── Tool ring ──
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
            start,
            end,
        );

        // Background
        if is_hovered {
            ctx.set_source_rgba(0.30, 0.50, 0.80, 0.85);
        } else if is_active {
            ctx.set_source_rgba(0.22, 0.35, 0.55, 0.85);
        } else {
            ctx.set_source_rgba(0.12, 0.15, 0.22, 0.88);
        }
        let _ = ctx.fill_preserve();

        // Border
        if is_active {
            ctx.set_source_rgba(0.45, 0.65, 0.95, 0.85);
            ctx.set_line_width(2.0);
        } else {
            ctx.set_source_rgba(0.30, 0.35, 0.45, 0.7);
            ctx.set_line_width(1.0);
        }
        let _ = ctx.stroke();

        // Label
        let mid_angle = start + seg_angle / 2.0;
        let mid_r = (layout.tool_inner + layout.tool_outer) / 2.0;
        let lx = cx + mid_r * mid_angle.cos();
        let ly = cy + mid_r * mid_angle.sin();
        draw_centered_label(ctx, lx, ly, label, 11.0, is_hovered);
    }

    // ── Center circle ──
    let is_center_hovered = hover == Some(RadialSegmentId::Center);
    ctx.new_path();
    ctx.arc(cx, cy, layout.center_radius, 0.0, 2.0 * PI);
    if is_center_hovered {
        ctx.set_source_rgba(0.25, 0.30, 0.40, 0.95);
    } else {
        ctx.set_source_rgba(0.10, 0.13, 0.18, 0.95);
    }
    let _ = ctx.fill_preserve();

    // Color indicator ring around center
    ctx.set_source_rgba(
        input_state.current_color.r,
        input_state.current_color.g,
        input_state.current_color.b,
        0.9,
    );
    ctx.set_line_width(3.0);
    let _ = ctx.stroke();

    // Current tool label in center
    let tool_label = active_tool_short_label(active_tool, input_state);
    draw_centered_label(ctx, cx, cy - 2.0, tool_label, 10.0, false);

    // Thickness gauge (small arc near bottom of center)
    draw_thickness_gauge(ctx, &layout, input_state);

    let _ = ctx.restore();
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
    highlighted: bool,
) {
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(size);
    let Ok(extents) = ctx.text_extents(text) else {
        return;
    };
    let tx = x - extents.width() / 2.0 - extents.x_bearing();
    let ty = y - extents.height() / 2.0 - extents.y_bearing();

    if highlighted {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    } else {
        ctx.set_source_rgba(0.85, 0.88, 0.93, 0.95);
    }
    ctx.move_to(tx, ty);
    let _ = ctx.show_text(text);
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
    ctx.set_source_rgba(0.4, 0.4, 0.45, 0.5);
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
        ctx.set_source_rgba(0.45, 0.70, 1.0, 0.8);
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
        4 => tool == Tool::Rect || tool == Tool::Ellipse,
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
        Tool::Eraser => "Eraser",
        Tool::Select => "Select",
        Tool::Highlight => "Highlight",
        Tool::StepMarker => "Step",
    }
}
