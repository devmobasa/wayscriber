use smithay_client_toolkit::{
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, LayerSurface},
    },
    shm::slot::SlotPool,
};
use wayland_client::{Proxy, protocol::wl_surface};
use wayland_protocols::wp::{
    fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1,
    viewporter::client::wp_viewport::WpViewport,
};

use crate::backend::wayland::toolbar::events::ToolbarCursorHint;
use crate::backend::wayland::toolbar::hit::HitRegion;

#[derive(Debug)]
pub struct ToolbarSurface {
    pub name: &'static str,
    pub anchor: Anchor,
    pub margin: (i32, i32, i32, i32), // top, right, bottom, left
    pub logical_size: (u32, u32),
    pub(super) wl_surface: Option<wl_surface::WlSurface>,
    pub(crate) layer_surface: Option<LayerSurface>,
    pub(super) pool: Option<SlotPool>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: i32,
    pub(super) preferred_scale: Option<u32>,
    pub(super) fractional_scale: Option<WpFractionalScaleV1>,
    pub(super) viewport: Option<WpViewport>,
    pub(super) ui_scale: f64,
    pub(crate) configured: bool,
    pub(super) dirty: bool,
    /// Consecutive failed renders, so a surface that cannot draw reports the
    /// reason without repeating it on every frame.
    pub(super) render_failures: u32,
    pub(super) suppressed: bool,
    /// Surface-local rects that accept input; None means the whole surface.
    /// Set when the drawn content does not cover the surface (popovers,
    /// overflow menus) so transparent areas pass clicks to the canvas.
    pub(super) input_rects: Option<Vec<(f64, f64, f64, f64)>>,
    pub(super) input_region_dirty: bool,
    pub(super) hit_regions: Vec<HitRegion>,
    pub(super) hover: Option<(f64, f64)>,
    pub(super) focus_index: Option<usize>,
    pub(super) focus_id: Option<String>,
}

impl ToolbarSurface {
    pub fn new(name: &'static str, anchor: Anchor, margin: (i32, i32, i32, i32)) -> Self {
        Self {
            name,
            anchor,
            margin,
            logical_size: (0, 0),
            wl_surface: None,
            layer_surface: None,
            pool: None,
            width: 0,
            height: 0,
            scale: 1,
            preferred_scale: None,
            fractional_scale: None,
            viewport: None,
            ui_scale: 1.0,
            configured: false,
            dirty: false,
            render_failures: 0,
            suppressed: false,
            input_rects: None,
            input_region_dirty: false,
            hit_regions: Vec::new(),
            hover: None,
            focus_index: None,
            focus_id: None,
        }
    }

    pub fn is_layer(&self, layer: &LayerSurface) -> bool {
        self.layer_surface
            .as_ref()
            .map(|ls| ls.wl_surface().id() == layer.wl_surface().id())
            .unwrap_or(false)
    }

    pub(in crate::backend::wayland) fn wl_surface(&self) -> Option<&wl_surface::WlSurface> {
        self.wl_surface.as_ref()
    }

    pub fn is_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.wl_surface
            .as_ref()
            .map(|s| s.id() == surface.id())
            .unwrap_or(false)
    }

    pub(in crate::backend::wayland) fn set_preferred_scale(
        &mut self,
        source: &WpFractionalScaleV1,
        preferred: u32,
    ) -> anyhow::Result<bool> {
        if preferred == 0
            || self.fractional_scale.as_ref() != Some(source)
            || self.preferred_scale == Some(preferred)
        {
            return Ok(false);
        }
        if self.width > 0 && self.height > 0 {
            crate::backend::wayland::surface_geometry::SurfaceGeometry::new(
                self.width,
                self.height,
                self.scale,
                Some(preferred),
                1,
            )?;
        }
        self.preferred_scale = Some(preferred);
        log::info!(
            "Preferred fractional scale {preferred}/120 for {} toolbar",
            self.name
        );
        self.pool = None;
        self.dirty = true;
        Ok(true)
    }

    /// Get cursor hint for the current hover position.
    pub fn cursor_hint(&self) -> Option<ToolbarCursorHint> {
        let (hx, hy) = self.hover?;
        let hint =
            crate::backend::wayland::toolbar::hit::resolve_hit_index(&self.hit_regions, hx, hy)
                .map_or(ToolbarCursorHint::Default, |index| {
                    self.hit_regions[index].kind.cursor_hint()
                });
        Some(hint)
    }
}
