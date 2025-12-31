pub(in crate::backend::wayland::toolbar::render) fn draw_label_center(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_label_center_color(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
    color: (f64, f64, f64, f64),
) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(color.0, color.1, color.2, color.3);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_label_left(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    _w: f64,
    h: f64,
    text: &str,
) {
    if let Ok(ext) = ctx.text_extents(text) {
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(x, ty);
        let _ = ctx.show_text(text);
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_section_label(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
) {
    ctx.set_source_rgba(0.8, 0.8, 0.85, 0.9);
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
}
