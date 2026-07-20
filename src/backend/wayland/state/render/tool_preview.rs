use super::ui_effect_damage::effect_rect;
use crate::draw::Color;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::theme::{self, overlay};
use crate::ui::toolbar::model::{self, SemanticToolIcon};
use crate::util::Rect;

/// A trailing bubble showing the active tool (glyph), its draw color, and its
/// width (a dot sized to the current thickness with an auto-contrast outline).
/// `thickness` is the active tool's stroke width (eraser size for the eraser).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tool_preview(
    ctx: &cairo::Context,
    tool: Tool,
    color: Color,
    thickness: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let (bx, by, bubble_w, bubble_h) = tool_preview_bubble(thickness, x, y, w, h);
    let glyph = overlay::CURSOR_PREVIEW_GLYPH_SIZE;
    let pad = overlay::CURSOR_PREVIEW_PAD;
    let gap = overlay::CURSOR_PREVIEW_GAP;
    let dot_d = tool_preview_dot_diameter(thickness);

    let _ = ctx.save();

    // Chrome: soft drop shadow + fill + hairline, matching the floating islands
    // so the preview reads as part of the same surface family.
    let radius = overlay::RADIUS_MD;
    theme::set_color(ctx, overlay::SHADOW_DEEP);
    rounded_rect_path(ctx, bx + 1.0, by + 1.5, bubble_w, bubble_h, radius);
    let _ = ctx.fill();
    theme::set_color(ctx, overlay::CURSOR_PREVIEW_BG);
    rounded_rect_path(ctx, bx, by, bubble_w, bubble_h, radius);
    let _ = ctx.fill();
    theme::set_color(ctx, overlay::CURSOR_PREVIEW_BORDER);
    ctx.set_line_width(1.0);
    rounded_rect_path(
        ctx,
        bx + 0.5,
        by + 0.5,
        bubble_w - 1.0,
        bubble_h - 1.0,
        radius - 0.5,
    );
    let _ = ctx.stroke();

    // Tools with no meaningful draw color render neutral for the width dot;
    // drawing tools carry their current color on it.
    let neutral = matches!(tool, Tool::Eraser | Tool::Select);

    // Tool glyph on the left, vertically centered, in the neutral foreground so
    // the tool identity reads regardless of the draw color.
    let glyph_x = bx + pad;
    let glyph_y = by + (bubble_h - glyph) / 2.0;
    theme::set_color(ctx, overlay::CURSOR_PREVIEW_NEUTRAL);
    draw_semantic_tool_icon(
        ctx,
        model::semantic_icon_for_tool(tool),
        glyph_x,
        glyph_y,
        glyph,
    );

    // Width dot on the right, in the current color at the current width.
    let (dr, dg, db) = if neutral {
        (
            overlay::CURSOR_PREVIEW_NEUTRAL.0,
            overlay::CURSOR_PREVIEW_NEUTRAL.1,
            overlay::CURSOR_PREVIEW_NEUTRAL.2,
        )
    } else {
        (color.r, color.g, color.b)
    };
    let dot_cx = bx + pad + glyph + gap + dot_d / 2.0;
    let dot_cy = by + bubble_h / 2.0;
    let dot_r = dot_d / 2.0;
    ctx.set_source_rgba(dr, dg, db, 1.0);
    ctx.arc(dot_cx, dot_cy, dot_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Auto-contrast outline keeps the dot legible against both its own color
    // and the dark bubble (see `theme::cursor_preview_outline`).
    theme::set_color(ctx, theme::cursor_preview_outline(dr, dg, db));
    ctx.set_line_width(1.0);
    ctx.arc(dot_cx, dot_cy, dot_r + 0.5, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();

    let _ = ctx.restore();
}

/// Trace a rounded-rectangle sub-path (mirrors `ui::primitives::draw_rounded_rect`,
/// which is private to the `ui` module and unreachable from the render layer).
fn rounded_rect_path(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    ctx.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    ctx.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    ctx.close_path();
}

/// Rendered diameter of the width dot: the active thickness, clamped so the
/// bubble stays compact (and within the cursor damage radius) for very thick
/// or hairline strokes.
fn tool_preview_dot_diameter(thickness: f64) -> f64 {
    thickness.clamp(
        overlay::CURSOR_PREVIEW_DOT_MIN,
        overlay::CURSOR_PREVIEW_DOT_MAX,
    )
}

/// Bubble origin and size for the preview: it trails the pointer by
/// [`overlay::CURSOR_PREVIEW_OFFSET`], clamped to stay fully on-screen.
fn tool_preview_bubble(thickness: f64, x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
    let glyph = overlay::CURSOR_PREVIEW_GLYPH_SIZE;
    let pad = overlay::CURSOR_PREVIEW_PAD;
    let gap = overlay::CURSOR_PREVIEW_GAP;
    let dot_d = tool_preview_dot_diameter(thickness);

    let bubble_w = glyph + gap + dot_d + pad * 2.0;
    let bubble_h = glyph.max(dot_d) + pad * 2.0;

    let bx = (x + overlay::CURSOR_PREVIEW_OFFSET).clamp(4.0, (w - bubble_w - 4.0).max(4.0));
    let by = (y + overlay::CURSOR_PREVIEW_OFFSET).clamp(4.0, (h - bubble_h - 4.0).max(4.0));
    (bx, by, bubble_w, bubble_h)
}

/// The idle-motion redraw the pointer handler owes the mouse-anchored tool
/// preview: whether a redraw is warranted and, if so, the screen-space damage
/// that lets the bubble track the cursor.
pub(super) struct MouseToolPreviewRedraw {
    /// True when the bubble moved and the surface must be repainted.
    pub redraw: bool,
    /// Union of the previous and current bubble footprints (old + new) so the
    /// stale bubble is cleared and the new one painted. Empty when no redraw
    /// is needed, or (degenerate surface only) when the footprints are unknown.
    pub rects: Vec<Rect>,
}

pub(super) struct MouseToolPreviewDamageUpdate {
    pub current: Option<Rect>,
    pub rects: Vec<Rect>,
}

/// Per-frame damage update for the preview bubble. Unlike the pointer-motion
/// helper, this accepts the last rendered footprint explicitly, so a visible
/// preview becoming hidden still damages and clears its old pixels.
pub(super) fn mouse_tool_preview_damage_update(
    previous: Option<Rect>,
    active: bool,
    thickness: f64,
    position: (f64, f64),
    width: u32,
    height: u32,
) -> MouseToolPreviewDamageUpdate {
    let current = active
        .then(|| mouse_tool_preview_damage_rect(thickness, position, width, height))
        .flatten();
    let mut rects = Vec::with_capacity(2);
    match (previous, current) {
        (Some(previous), Some(current)) if previous == current => rects.push(current),
        (previous, current) => {
            rects.extend(previous);
            rects.extend(current);
        }
    }
    MouseToolPreviewDamageUpdate { current, rects }
}

/// Damage the mouse tool-preview bubble needs to follow the pointer from
/// `prev` to `next`.
///
/// `eligible` mirrors the render-time gate
/// ([`WaylandState::mouse_tool_preview_eligible`]); the bubble is drawn from
/// the same origin the render pass uses (see [`draw_tool_preview`] /
/// [`tool_preview_bubble`]), so damaging both footprints keeps the two in
/// lockstep. Ineligible previews, and moves too small to shift the (clamped)
/// bubble, request no redraw.
pub(super) fn mouse_tool_preview_redraw(
    eligible: bool,
    thickness: f64,
    prev: (f64, f64),
    next: (f64, f64),
    width: u32,
    height: u32,
) -> MouseToolPreviewRedraw {
    if !eligible {
        return MouseToolPreviewRedraw {
            redraw: false,
            rects: Vec::new(),
        };
    }
    let prev_rect = mouse_tool_preview_damage_rect(thickness, prev, width, height);
    let next_rect = mouse_tool_preview_damage_rect(thickness, next, width, height);
    if prev_rect == next_rect {
        // Sub-pixel motion, or an edge clamp that pins the bubble in place:
        // the rendered preview is already correct, so skip a needless redraw.
        return MouseToolPreviewRedraw {
            redraw: false,
            rects: Vec::new(),
        };
    }
    let mut rects = Vec::with_capacity(2);
    rects.extend(prev_rect);
    rects.extend(next_rect);
    MouseToolPreviewRedraw {
        redraw: true,
        rects,
    }
}

/// Screen-space damage rect for the preview bubble anchored at `pos`, expanded
/// by the shared UI-effect anti-aliasing margin (matches the toast/HUD damage).
fn mouse_tool_preview_damage_rect(
    thickness: f64,
    pos: (f64, f64),
    width: u32,
    height: u32,
) -> Option<Rect> {
    let bounds = tool_preview_bubble(thickness, pos.0, pos.1, width as f64, height as f64);
    effect_rect(bounds, width, height)
}

fn draw_semantic_tool_icon(
    ctx: &cairo::Context,
    icon: SemanticToolIcon,
    x: f64,
    y: f64,
    size: f64,
) {
    match icon {
        SemanticToolIcon::Select => toolbar_icons::draw_icon_select(ctx, x, y, size),
        SemanticToolIcon::Pen => toolbar_icons::draw_icon_pen(ctx, x, y, size),
        SemanticToolIcon::Line => toolbar_icons::draw_icon_line(ctx, x, y, size),
        SemanticToolIcon::Rect => toolbar_icons::draw_icon_rect(ctx, x, y, size),
        SemanticToolIcon::Circle => toolbar_icons::draw_icon_circle(ctx, x, y, size),
        SemanticToolIcon::Triangle => toolbar_icons::draw_icon_triangle(ctx, x, y, size),
        SemanticToolIcon::Parallelogram => {
            toolbar_icons::draw_icon_parallelogram(ctx, x, y, size);
        }
        SemanticToolIcon::Rhombus => toolbar_icons::draw_icon_rhombus(ctx, x, y, size),
        SemanticToolIcon::Polygon => toolbar_icons::draw_icon_polygon(ctx, x, y, size),
        SemanticToolIcon::FreeformPolygon => {
            toolbar_icons::draw_icon_freeform_polygon(ctx, x, y, size);
        }
        SemanticToolIcon::Arrow => toolbar_icons::draw_icon_arrow(ctx, x, y, size),
        SemanticToolIcon::Blur => toolbar_icons::draw_icon_blur(ctx, x, y, size),
        SemanticToolIcon::Marker => toolbar_icons::draw_icon_marker(ctx, x, y, size),
        SemanticToolIcon::Highlight => toolbar_icons::draw_icon_highlight(ctx, x, y, size),
        SemanticToolIcon::StepMarker => toolbar_icons::draw_icon_step_marker(ctx, x, y, size),
        SemanticToolIcon::Eraser => toolbar_icons::draw_icon_eraser(ctx, x, y, size),
    }
}

pub(super) fn draw_stylus_hover_cursor(
    ctx: &cairo::Context,
    tool: Tool,
    color: Color,
    x: f64,
    y: f64,
) {
    let (r, g, b, radius) = match tool {
        Tool::Eraser | Tool::Select => (0.96, 0.96, 0.98, 4.0),
        _ => (color.r, color.g, color.b, 3.5),
    };

    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ctx.arc(
        x + 1.0,
        y + 1.0,
        radius + 2.0,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.arc(x, y, radius + 1.4, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(r, g, b, 0.95);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.72);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 1.4, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, Format, ImageSurface};

    #[test]
    fn dot_diameter_clamps_thickness_to_bounds() {
        assert_eq!(
            tool_preview_dot_diameter(0.5),
            overlay::CURSOR_PREVIEW_DOT_MIN,
            "hairline strokes still render a visible dot"
        );
        assert_eq!(
            tool_preview_dot_diameter(200.0),
            overlay::CURSOR_PREVIEW_DOT_MAX,
            "very thick strokes cap so the bubble stays compact"
        );
        let mid = (overlay::CURSOR_PREVIEW_DOT_MIN + overlay::CURSOR_PREVIEW_DOT_MAX) / 2.0;
        assert_eq!(
            tool_preview_dot_diameter(mid),
            mid,
            "in-range width is exact"
        );
    }

    #[test]
    fn bubble_grows_with_thickness() {
        let (_, _, thin_w, _) = tool_preview_bubble(2.0, 100.0, 100.0, 800.0, 600.0);
        let (_, _, thick_w, _) = tool_preview_bubble(200.0, 100.0, 100.0, 800.0, 600.0);
        assert!(
            thick_w > thin_w,
            "a thicker stroke widens the bubble to fit a larger dot"
        );
    }

    #[test]
    fn bubble_trails_the_pointer_but_stays_on_screen() {
        // Well inside the surface: bubble sits offset from the hotspot.
        let (bx, by, _, _) = tool_preview_bubble(4.0, 100.0, 100.0, 800.0, 600.0);
        assert_eq!(bx, 100.0 + overlay::CURSOR_PREVIEW_OFFSET);
        assert_eq!(by, 100.0 + overlay::CURSOR_PREVIEW_OFFSET);

        // Bottom-right corner: bubble is pulled back so it never leaves the
        // surface.
        let (bx, by, bubble_w, bubble_h) = tool_preview_bubble(4.0, 799.0, 599.0, 800.0, 600.0);
        assert!(bx + bubble_w <= 800.0, "clamped within right edge");
        assert!(by + bubble_h <= 600.0, "clamped within bottom edge");
    }

    /// Little-endian ARgb32 stores premultiplied BGRA in memory.
    fn has_strong_green_pixel(pixels: &[u8]) -> bool {
        pixels.chunks_exact(4).any(|px| {
            let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
            g > 200 && r < 60 && b < 60 && a > 200
        })
    }

    #[test]
    fn draw_tool_preview_paints_the_current_color_dot() {
        let surface = ImageSurface::create(Format::ARgb32, 96, 96).expect("surface");
        let ctx = Context::new(&surface).expect("context");
        let green = Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };

        draw_tool_preview(&ctx, Tool::Pen, green, 8.0, 20.0, 20.0, 96.0, 96.0);
        surface.flush();

        let mut found = false;
        surface
            .with_data(|pixels| found = has_strong_green_pixel(pixels))
            .expect("surface data");
        assert!(
            found,
            "the width dot must render in the active draw color (green)"
        );
    }

    #[test]
    fn mouse_preview_ineligible_requests_no_redraw() {
        let out = mouse_tool_preview_redraw(false, 8.0, (100.0, 100.0), (400.0, 400.0), 800, 600);
        assert!(!out.redraw, "an ineligible preview never asks for a redraw");
        assert!(out.rects.is_empty());
    }

    #[test]
    fn mouse_preview_visibility_loss_damages_the_last_visible_bounds() {
        let position = (100.0, 100.0);
        let previous = effect_rect(
            tool_preview_bubble(8.0, position.0, position.1, 800.0, 600.0),
            800,
            600,
        )
        .expect("previous footprint");
        let update =
            mouse_tool_preview_damage_update(Some(previous), false, 8.0, position, 800, 600);

        assert!(update.current.is_none());
        assert_eq!(update.rects, vec![previous]);
    }

    #[test]
    fn mouse_preview_motion_damages_old_and_new_bounds() {
        // The pointer-to-render path: an eligible preview that moves must
        // request a redraw and damage BOTH the previous and current bubble
        // footprints so the trailing bubble follows the cursor.
        let prev = (100.0, 100.0);
        let next = (400.0, 400.0);
        let out = mouse_tool_preview_redraw(true, 8.0, prev, next, 800, 600);
        assert!(out.redraw, "idle motion with the preview enabled redraws");

        let expect_prev = effect_rect(
            tool_preview_bubble(8.0, prev.0, prev.1, 800.0, 600.0),
            800,
            600,
        )
        .expect("prev footprint");
        let expect_next = effect_rect(
            tool_preview_bubble(8.0, next.0, next.1, 800.0, 600.0),
            800,
            600,
        )
        .expect("next footprint");
        assert_ne!(
            expect_prev, expect_next,
            "the two footprints must differ for this to exercise old+new"
        );
        assert_eq!(
            out.rects,
            vec![expect_prev, expect_next],
            "damage covers the stale bubble and the new bubble"
        );
    }

    #[test]
    fn mouse_preview_without_motion_requests_no_redraw() {
        let out = mouse_tool_preview_redraw(true, 8.0, (100.0, 100.0), (100.0, 100.0), 800, 600);
        assert!(
            !out.redraw,
            "a pointer that did not move the bubble does not redraw"
        );
        assert!(out.rects.is_empty());
    }

    #[test]
    fn draw_tool_preview_neutralizes_color_for_eraser() {
        let surface = ImageSurface::create(Format::ARgb32, 96, 96).expect("surface");
        let ctx = Context::new(&surface).expect("context");
        // Eraser has no meaningful draw color; even a green setting renders
        // neutral, so no strong-green dot appears.
        let green = Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };

        draw_tool_preview(&ctx, Tool::Eraser, green, 8.0, 20.0, 20.0, 96.0, 96.0);
        surface.flush();

        let mut found = false;
        surface
            .with_data(|pixels| found = has_strong_green_pixel(pixels))
            .expect("surface data");
        assert!(
            !found,
            "eraser preview ignores the stored color and renders neutral"
        );
    }
}
