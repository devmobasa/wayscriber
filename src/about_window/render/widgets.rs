pub(super) fn draw_copy_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.85, 0.9, 0.98)
    } else {
        (0.92, 0.94, 0.96)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.55, 0.6, 0.68);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    let pad = 3.0;
    let icon_size = size - pad * 2.0;
    let back = (x + pad + 2.0, y + pad - 1.0);
    let front = (x + pad - 1.0, y + pad + 2.0);
    ctx.set_source_rgb(0.35, 0.4, 0.48);
    draw_rounded_rect(ctx, back.0, back.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();
    draw_rounded_rect(ctx, front.0, front.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();

    (x, y, size, size)
}

pub(super) fn draw_close_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.98, 0.88, 0.88)
    } else {
        (0.96, 0.92, 0.92)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.7, 0.55, 0.55);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    ctx.set_source_rgb(0.4, 0.25, 0.25);
    ctx.set_line_width(1.6);
    let inset = 4.0;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();

    (x, y, size, size)
}

fn draw_rounded_rect(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let r = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + width - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(
        x + width - r,
        y + height - r,
        r,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    ctx.arc(
        x + r,
        y + height - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}
