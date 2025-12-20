use smithay_client_toolkit::compositor::{CompositorState, Region};
use wayland_client::protocol::wl_surface::WlSurface;

pub fn set_surface_clickthrough(
    compositor: &CompositorState,
    surface: &WlSurface,
    clickthrough: bool,
) {
    if clickthrough {
        if let Ok(region) = Region::new(compositor) {
            surface.set_input_region(Some(region.wl_region()));
        }
    } else {
        surface.set_input_region(None);
    }
    surface.commit();
}
