use smithay_client_toolkit::compositor::{CompositorState, Region};
use wayland_client::protocol::wl_surface::WlSurface;

use crate::util::Rect;

pub fn set_surface_clickthrough(
    compositor: &CompositorState,
    surface: &WlSurface,
    clickthrough: bool,
    hotspot: Option<Rect>,
) {
    if clickthrough {
        if let Ok(region) = Region::new(compositor) {
            if let Some(rect) = hotspot {
                region.add(rect.x, rect.y, rect.width, rect.height);
            }
            surface.set_input_region(Some(region.wl_region()));
        }
    } else {
        surface.set_input_region(None);
    }
    surface.commit();
}
