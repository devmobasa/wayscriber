use cairo::Context;
use std::f64::consts::{FRAC_1_SQRT_2, PI};

/// Draw a clear/trash icon
pub fn draw_icon_clear(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_clear_canvas(ctx, x, y, size);
}

/// Draw a freeze/pause icon
pub fn draw_icon_freeze(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Snowflake - vertical line
    ctx.move_to(x + s * 0.5, y + s * 0.15);
    ctx.line_to(x + s * 0.5, y + s * 0.85);
    let _ = ctx.stroke();

    // Snowflake - diagonal lines
    ctx.move_to(x + s * 0.2, y + s * 0.32);
    ctx.line_to(x + s * 0.8, y + s * 0.68);
    let _ = ctx.stroke();

    ctx.move_to(x + s * 0.8, y + s * 0.32);
    ctx.line_to(x + s * 0.2, y + s * 0.68);
    let _ = ctx.stroke();

    // Small branches
    ctx.set_line_width(stroke * 0.7);
    ctx.move_to(x + s * 0.5, y + s * 0.3);
    ctx.line_to(x + s * 0.4, y + s * 0.2);
    ctx.move_to(x + s * 0.5, y + s * 0.3);
    ctx.line_to(x + s * 0.6, y + s * 0.2);
    let _ = ctx.stroke();
}

/// Draw an unfreeze/play icon
pub fn draw_icon_unfreeze(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    ctx.set_line_join(cairo::LineJoin::Round);

    // Play triangle
    ctx.move_to(x + s * 0.3, y + s * 0.2);
    ctx.line_to(x + s * 0.3, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.5);
    ctx.close_path();
    let _ = ctx.fill();
}

/// Draw a settings/gear icon
pub fn draw_icon_settings(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Inner circle
    ctx.arc(cx, cy, s * 0.15, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Outer gear teeth (6 teeth)
    let inner_r = s * 0.25;
    let outer_r = s * 0.38;
    for i in 0..6 {
        let angle = (i as f64) * PI / 3.0;
        let x1 = cx + angle.cos() * inner_r;
        let y1 = cy + angle.sin() * inner_r;
        let x2 = cx + angle.cos() * outer_r;
        let y2 = cy + angle.sin() * outer_r;
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        let _ = ctx.stroke();
    }

    // Outer circle
    ctx.arc(cx, cy, s * 0.32, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw a document/file icon
pub fn draw_icon_file(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Document outline with folded corner
    ctx.move_to(x + s * 0.25, y + s * 0.15);
    ctx.line_to(x + s * 0.6, y + s * 0.15);
    ctx.line_to(x + s * 0.75, y + s * 0.3);
    ctx.line_to(x + s * 0.75, y + s * 0.85);
    ctx.line_to(x + s * 0.25, y + s * 0.85);
    ctx.close_path();
    let _ = ctx.stroke();

    // Folded corner
    ctx.move_to(x + s * 0.6, y + s * 0.15);
    ctx.line_to(x + s * 0.6, y + s * 0.3);
    ctx.line_to(x + s * 0.75, y + s * 0.3);
    let _ = ctx.stroke();

    // Text lines
    ctx.move_to(x + s * 0.35, y + s * 0.5);
    ctx.line_to(x + s * 0.65, y + s * 0.5);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.35, y + s * 0.65);
    ctx.line_to(x + s * 0.65, y + s * 0.65);
    let _ = ctx.stroke();
}

/// Draw a floppy disk/save icon
pub fn draw_icon_save(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    let pad = s * 0.15;
    let body_x = x + pad;
    let body_y = y + pad;
    let body_w = s - pad * 2.0;
    let body_h = s - pad * 2.0;
    let notch_w = s * 0.22;
    let notch_h = s * 0.18;

    // Outer body with a top-right notch.
    ctx.move_to(body_x, body_y);
    ctx.line_to(body_x + body_w - notch_w, body_y);
    ctx.line_to(body_x + body_w, body_y + notch_h);
    ctx.line_to(body_x + body_w, body_y + body_h);
    ctx.line_to(body_x, body_y + body_h);
    ctx.close_path();
    let _ = ctx.stroke();

    // Shutter tab near the top.
    let shutter_w = body_w * 0.4;
    let shutter_h = body_h * 0.18;
    let shutter_x = body_x + body_w * 0.1;
    let shutter_y = body_y + body_h * 0.12;
    ctx.rectangle(shutter_x, shutter_y, shutter_w, shutter_h);
    let _ = ctx.stroke();

    // Label window.
    let label_w = body_w * 0.55;
    let label_h = body_h * 0.22;
    let label_x = body_x + (body_w - label_w) / 2.0;
    let label_y = body_y + body_h - label_h - body_h * 0.12;
    ctx.rectangle(label_x, label_y, label_w, label_h);
    let _ = ctx.stroke();
}

/// Draw a stacked-boards / layers icon (the Canvas popover entry): a diamond
/// top plate above two nested chevron lines standing in for the layers below.
pub fn draw_icon_layers(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    let cx = x + s * 0.5;
    let half_w = s * 0.35;
    let half_h = s * 0.18;

    // Top plate: a rhombus (rotated square).
    let top_y = y + s * 0.14;
    ctx.move_to(cx, top_y);
    ctx.line_to(cx + half_w, top_y + half_h);
    ctx.line_to(cx, top_y + half_h * 2.0);
    ctx.line_to(cx - half_w, top_y + half_h);
    ctx.close_path();
    let _ = ctx.stroke();

    // Two chevron lines beneath, tracing where lower plates would sit.
    for offset in [s * 0.28, s * 0.46] {
        ctx.move_to(cx - half_w, top_y + half_h + offset);
        ctx.line_to(cx, top_y + half_h * 2.0 + offset);
        ctx.line_to(cx + half_w, top_y + half_h + offset);
        let _ = ctx.stroke();
    }
}

/// Draw a board-picker icon: a 2x2 grid of cells — the "browse all boards"
/// overview that opens the board picker.
pub fn draw_icon_grid(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    let pad = s * 0.16;
    let gap = s * 0.14;
    let cell = (s - pad * 2.0 - gap) / 2.0;
    for row in 0..2 {
        for col in 0..2 {
            let cx = x + pad + col as f64 * (cell + gap);
            let cy = y + pad + row as f64 * (cell + gap);
            ctx.rectangle(cx, cy, cell, cell);
            let _ = ctx.stroke();
        }
    }
}

/// Draw a session icon (the Session popover entry): a floppy-disk save body
/// with a small clock overlaid in the lower-right corner (save + recent).
pub fn draw_icon_session(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.09).max(1.4);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Floppy-disk body occupying the upper-left, leaving room for the clock.
    let pad = s * 0.14;
    let body_x = x + pad;
    let body_y = y + pad;
    let body_w = s * 0.56;
    let body_h = s * 0.56;
    let notch = s * 0.16;
    ctx.move_to(body_x, body_y);
    ctx.line_to(body_x + body_w - notch, body_y);
    ctx.line_to(body_x + body_w, body_y + notch);
    ctx.line_to(body_x + body_w, body_y + body_h);
    ctx.line_to(body_x, body_y + body_h);
    ctx.close_path();
    let _ = ctx.stroke();
    // Shutter tab near the top edge.
    ctx.rectangle(
        body_x + body_w * 0.14,
        body_y + body_h * 0.1,
        body_w * 0.4,
        body_h * 0.18,
    );
    let _ = ctx.stroke();

    // Clock badge in the lower-right corner.
    let clock_cx = x + s * 0.72;
    let clock_cy = y + s * 0.72;
    let clock_r = s * 0.22;
    ctx.arc(clock_cx, clock_cy, clock_r, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    ctx.move_to(clock_cx, clock_cy);
    ctx.line_to(clock_cx, clock_cy - clock_r * 0.6);
    let _ = ctx.stroke();
    ctx.move_to(clock_cx, clock_cy);
    ctx.line_to(clock_cx + clock_r * 0.55, clock_cy);
    let _ = ctx.stroke();
}

/// Draw a sliders / "tune" icon (the Settings popover entry): three
/// horizontal tracks each carrying a knob at a distinct position. Distinct
/// from the gear `draw_icon_settings` used for the session manager elsewhere.
pub fn draw_icon_sliders(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_cap(cairo::LineCap::Round);

    let left = x + s * 0.18;
    let right = x + s * 0.82;
    let knob_r = s * 0.09;
    // (row y-fraction, knob x-fraction along the track)
    for (row, knob_t) in [(0.28_f64, 0.65_f64), (0.5, 0.35), (0.72, 0.6)] {
        let ly = y + s * row;
        ctx.set_line_width(stroke);
        ctx.move_to(left, ly);
        ctx.line_to(right, ly);
        let _ = ctx.stroke();
        let knob_x = left + (right - left) * knob_t;
        ctx.arc(knob_x, ly, knob_r, 0.0, PI * 2.0);
        let _ = ctx.fill();
    }
}

/// Draw a clock/delay icon
#[allow(dead_code)]
pub fn draw_icon_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Clock circle
    ctx.arc(cx, cy, s * 0.35, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Clock hands
    ctx.move_to(cx, cy);
    ctx.line_to(cx, cy - s * 0.2);
    let _ = ctx.stroke();
    ctx.move_to(cx, cy);
    ctx.line_to(cx + s * 0.15, cy + s * 0.1);
    let _ = ctx.stroke();
}

/// Draw a refresh/reload icon (circular arrow).
#[allow(dead_code)]
pub fn draw_icon_refresh(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.7);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    let r = s * 0.30;
    let start = 0.30 * PI;
    let end = 1.85 * PI;

    ctx.arc(cx, cy, r, start, end);
    let _ = ctx.stroke();

    // Bold triangular head straddling the ring at the arc end and pointing along
    // the tangent (direction of travel). It is intentionally large and protrudes
    // past the stroke so the glyph reads as a rotating arrow — not a bare "C" —
    // even at the ~16px size used in the command palette. Offsetting radially, as
    // the original did, collapses the head onto the stroke and loses the arrow.
    let ex = cx + r * end.cos();
    let ey = cy + r * end.sin();
    let (ts, tc) = (end + PI / 2.0).sin_cos(); // tangent (clockwise)
    let (rs, rc) = end.sin_cos(); // radial (outward)
    let h = s * 0.34;
    let tip_x = ex + 0.55 * h * tc;
    let tip_y = ey + 0.55 * h * ts;
    let base_x = ex - 0.45 * h * tc;
    let base_y = ey - 0.45 * h * ts;
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(base_x + 0.5 * h * rc, base_y + 0.5 * h * rs);
    ctx.line_to(base_x - 0.5 * h * rc, base_y - 0.5 * h * rs);
    ctx.close_path();
    let _ = ctx.fill();
}

/// Draw a search / magnifying-glass icon (used for the command palette).
pub fn draw_icon_search(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.11).max(1.6);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Lens.
    let cx = x + s * 0.42;
    let cy = y + s * 0.42;
    let r = s * 0.24;
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Handle, running from the lower-right of the lens toward the corner.
    let start = r + stroke * 0.25;
    ctx.move_to(cx + start * FRAC_1_SQRT_2, cy + start * FRAC_1_SQRT_2);
    ctx.line_to(x + s * 0.85, y + s * 0.85);
    let _ = ctx.stroke();
}

/// Draw a copy/duplicate icon (two overlapping rectangles).
pub fn draw_icon_copy(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Back rectangle
    ctx.rectangle(x + s * 0.3, y + s * 0.15, s * 0.5, s * 0.55);
    let _ = ctx.stroke();

    // Front rectangle (overlapping)
    ctx.rectangle(x + s * 0.2, y + s * 0.3, s * 0.5, s * 0.55);
    let _ = ctx.stroke();
}

/// Draw a screenshot/camera icon.
pub fn draw_icon_screenshot(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_screenshot(ctx, x, y, size);
}

/// Draw a visibility/eye icon.
pub fn draw_icon_visibility(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.09).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    ctx.move_to(x + s * 0.14, cy);
    ctx.curve_to(
        x + s * 0.28,
        y + s * 0.25,
        x + s * 0.72,
        y + s * 0.25,
        x + s * 0.86,
        cy,
    );
    ctx.curve_to(
        x + s * 0.72,
        y + s * 0.75,
        x + s * 0.28,
        y + s * 0.75,
        x + s * 0.14,
        cy,
    );
    let _ = ctx.stroke();

    ctx.arc(cx, cy, s * 0.14, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw a left chevron/arrow icon for navigation.
pub fn draw_icon_chevron_left(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Chevron pointing left: >
    ctx.move_to(x + s * 0.6, y + s * 0.2);
    ctx.line_to(x + s * 0.35, y + s * 0.5);
    ctx.line_to(x + s * 0.6, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a right chevron/arrow icon for navigation.
pub fn draw_icon_chevron_right(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Chevron pointing right: <
    ctx.move_to(x + s * 0.4, y + s * 0.2);
    ctx.line_to(x + s * 0.65, y + s * 0.5);
    ctx.line_to(x + s * 0.4, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a downward chevron icon (expand below).
#[allow(dead_code)] // used by toolbar-gtk section controls when that feature is enabled
pub fn draw_icon_chevron_down(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    ctx.move_to(x + s * 0.2, y + s * 0.4);
    ctx.line_to(x + s * 0.5, y + s * 0.65);
    ctx.line_to(x + s * 0.8, y + s * 0.4);
    let _ = ctx.stroke();
}

/// Draw a pencil/edit icon.
#[allow(dead_code)]
pub fn draw_icon_pencil(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    ctx.set_line_join(cairo::LineJoin::Round);

    // Pencil laid along a 45° axis pointing to the lower-left, drawn as three
    // filled bands — eraser cap, wooden body, sharpened nib — separated by thin
    // unpainted gaps that stand in for the ferrule and the wood/graphite line.
    // Additive fills only (the gaps merely reveal the row background), so it is
    // safe on any surface and reads clearly at the ~16px palette size, where the
    // old thin-outline parallelogram with no point just looked like a bar.
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    let ax = x + s * 0.76; // axis origin = center of the eraser end
    let ay = y + s * 0.24;
    let (ux, uy) = (-inv_sqrt2, inv_sqrt2); // down the pencil (toward the nib)
    let (nx, ny) = (inv_sqrt2, inv_sqrt2); // across the pencil
    let hw = s * 0.135; // half width

    // Point on a pencil edge at axis distance `d` (icon units) and side `sign`.
    let side = |d: f64, sign: f64| {
        (
            ax + s * d * ux + sign * hw * nx,
            ay + s * d * uy + sign * hw * ny,
        )
    };
    let band = |d0: f64, d1: f64| {
        let (a0, b0) = (side(d0, -1.0), side(d0, 1.0));
        let (b1, a1) = (side(d1, 1.0), side(d1, -1.0));
        ctx.move_to(a0.0, a0.1);
        ctx.line_to(b0.0, b0.1);
        ctx.line_to(b1.0, b1.1);
        ctx.line_to(a1.0, a1.1);
        ctx.close_path();
        let _ = ctx.fill();
    };

    band(0.0, 0.14); // eraser cap
    band(0.20, 0.62); // wooden body (gap above it = the ferrule)

    // Sharpened nib converging to a point (gap above it = the graphite line).
    let (rr, ll) = (side(0.66, 1.0), side(0.66, -1.0));
    let tip = (ax + s * 0.92 * ux, ay + s * 0.92 * uy);
    ctx.move_to(rr.0, rr.1);
    ctx.line_to(tip.0, tip.1);
    ctx.line_to(ll.0, ll.1);
    ctx.close_path();
    let _ = ctx.fill();
}
