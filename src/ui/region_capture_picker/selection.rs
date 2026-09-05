use super::layout::normalized_rect;
use super::types::RegionCaptureWindowVisual;
use crate::input::state::RegionSelection;

const SCRIM: (f64, f64, f64, f64) = (0.02, 0.03, 0.05, 0.48);

pub(super) fn draw_scrim(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    effective_selection: Option<RegionSelection>,
) {
    ctx.set_source_rgba(SCRIM.0, SCRIM.1, SCRIM.2, SCRIM.3);
    ctx.rectangle(0.0, 0.0, width, height);
    if let Some(selection) = effective_selection {
        let (x, y, w, h) = normalized_rect(selection);
        ctx.rectangle(x, y, w, h);
        ctx.set_fill_rule(cairo::FillRule::EvenOdd);
    }
    let _ = ctx.fill();
    ctx.set_fill_rule(cairo::FillRule::Winding);
}

pub(super) fn draw_selection_frame(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    corner_arms: bool,
) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(1.0);
    ctx.rectangle(x + 0.5, y + 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
    let _ = ctx.stroke();

    if !corner_arms {
        return;
    }
    let arm = (w.min(h) / 4.0).clamp(4.0, 20.0);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Square);
    for (corner_x, corner_y, dx, dy) in [
        (x, y, 1.0, 1.0),
        (x + w, y, -1.0, 1.0),
        (x, y + h, 1.0, -1.0),
        (x + w, y + h, -1.0, -1.0),
    ] {
        ctx.move_to(corner_x + dx * arm, corner_y);
        ctx.line_to(corner_x, corner_y);
        ctx.line_to(corner_x, corner_y + dy * arm);
        let _ = ctx.stroke();
    }
}

pub(super) fn draw_window_target_frames(
    ctx: &cairo::Context,
    window: RegionCaptureWindowVisual<'_>,
) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.34);
    ctx.set_line_width(1.0);
    for target in window.targets {
        let (x, y, width, height) = normalized_rect(*target);
        ctx.rectangle(
            x + 0.5,
            y + 0.5,
            (width - 1.0).max(0.0),
            (height - 1.0).max(0.0),
        );
        let _ = ctx.stroke();
    }
}

pub(super) fn draw_crosshair(ctx: &cairo::Context, pointer: (f64, f64), screen: (f64, f64)) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.22);
    ctx.set_line_width(1.0);
    ctx.move_to(0.0, pointer.1 + 0.5);
    ctx.line_to(screen.0, pointer.1 + 0.5);
    ctx.move_to(pointer.0 + 0.5, 0.0);
    ctx.line_to(pointer.0 + 0.5, screen.1);
    let _ = ctx.stroke();
}
