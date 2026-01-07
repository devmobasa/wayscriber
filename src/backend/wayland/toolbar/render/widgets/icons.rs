use super::constants::{COLOR_ICON_DEFAULT, COLOR_ICON_HOVER, set_color};

pub(in crate::backend::wayland::toolbar::render) fn set_icon_color(
    ctx: &cairo::Context,
    hover: bool,
) {
    set_color(
        ctx,
        if hover {
            COLOR_ICON_HOVER
        } else {
            COLOR_ICON_DEFAULT
        },
    );
}
