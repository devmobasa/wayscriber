pub(in crate::backend::wayland::toolbar::render) fn set_icon_color(
    ctx: &cairo::Context,
    hover: bool,
) {
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    } else {
        ctx.set_source_rgba(0.95, 0.95, 0.95, 0.9);
    }
}
